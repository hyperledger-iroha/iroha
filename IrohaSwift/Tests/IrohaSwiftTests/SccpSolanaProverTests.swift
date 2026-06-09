import CryptoKit
import Foundation
import XCTest
@testable import IrohaSwift

final class SccpSolanaProverTests: XCTestCase {
    private static let ethereumSyncCommitteeSupermajorityBits =
        "0x" + String(repeating: "ff", count: 42) + "3f" + String(repeating: "00", count: 21)
    private static let ethereumSyncCommitteeSupermajorityParticipation = "342"
    private static let ethereumFinalityBranch =
        (0..<6).map { "0x" + String(repeating: String(format: "%02x", 0x50 + $0), count: 32) }

    private static func ethereumSyncCommitteeBytes(_ byte: UInt8, count: Int) -> [Data] {
        (0..<512).map { index in
            var value = Data(repeating: byte, count: count)
            value[count - 2] = UInt8((index >> 8) & 0xff)
            value[count - 1] = UInt8(index & 0xff)
            return value
        }
    }

    private static func ethereumSyncCommitteeWeights() -> [UInt64] {
        Array(repeating: 1, count: 512)
    }

    private static func ethereumSyncCommitteeSignersBitmap(_ count: Int) -> Data {
        var bitmap = Data(repeating: 0, count: 64)
        for index in 0..<count {
            let byteIndex = index / 8
            bitmap[byteIndex] = bitmap[byteIndex] | UInt8(1 << (index % 8))
        }
        return bitmap
    }

    private static func sampleEthereumSyncCommitteePayload(publicKeyByte: UInt8 = 0x33,
                                                           popByte: UInt8 = 0xcc) throws -> Data {
        try canonicalEthSyncCommitteePayloadBytes(
            syncCommitteePublicKeys: ethereumSyncCommitteeBytes(publicKeyByte, count: 48),
            syncCommitteeWeights: ethereumSyncCommitteeWeights(),
            syncCommitteePops: ethereumSyncCommitteeBytes(popByte, count: 96)
        )
    }

    private static let solanaSignature55 =
        "2hxGyn4y9Mjkii76BqmxVoNYbTs3tw97bmtZRXnDoZPAw7VZTWhhk1aV11DtFgYGVibPaty4PQLHVLaKrT24NxGU"
    private static let solanaSignature01 =
        "2AXDGYSE4f2sz7tvMMzyHvUfcoJmxudvdhBcmiUSo6ijwfYmfZYsKRxboQMPh3R4kUhXRVdtSXFXMheka4Rc4P2"
    private static let solanaZeroSignature = String(repeating: "1", count: 64)
    private static let solanaProgram42 = "5TeWSsjg2gbxCyWVniXeCmwM7UtHTCK7svzJr5xYJzHf"
    private static let solanaProgram02 = "8qbHbw2BbbTHBW1sbeqakYXVKRQM8Ne7pLK7m6CVfeR"
    private static let solanaZeroProgram = String(repeating: "1", count: 32)
    private static let solanaMainnetGenesisPublicInput =
        "0x8dbaadfbc441ded0257a4700cd26d814b5a196be44b963454cff8dd9543f13b5"

    func testSolanaRouteCanaryEvidenceBindsProgramdataSnapshot() throws {
        let evidence = try Self.sampleSolanaRouteCanaryEvidence()

        XCTAssertEqual(try canonicalSolanaSccpRouteCanaryEvidenceBytes(evidence).count, 475)
        XCTAssertEqual(
            try solanaSccpRouteCanaryEvidenceHash(evidence),
            "0x77296e47d5681f97136dc79d66dbda4478c3c5ec80271bfd4f1f3b3dbb8e15ca"
        )
        XCTAssertEqual(sccpSolanaUpgradeableLoaderId, "BPFLoaderUpgradeab1e11111111111111111111111")

        XCTAssertThrowsError(try solanaSccpRouteCanaryEvidenceHash(
            try Self.sampleSolanaRouteCanaryEvidence(solanaProgramdataSlot: "4322")
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("solanaExpectedProgramdataSlot"))
        }
        XCTAssertThrowsError(try solanaSccpRouteCanaryEvidenceHash(
            try Self.sampleSolanaRouteCanaryEvidence(solanaProgramdataExecutableBase64: "AQIDBA==")
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("solanaProgramdataExecutable"))
        }
        XCTAssertThrowsError(try solanaSccpRouteCanaryEvidenceHash(
            try Self.sampleSolanaRouteCanaryEvidence(destinationBindingHash: String(repeating: "78", count: 32))
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("destinationBindingHash"))
        }
        XCTAssertThrowsError(try solanaSccpRouteCanaryEvidenceHash(
            try Self.sampleSolanaRouteCanaryEvidence(expectedDestinationBindingHash: String(repeating: "78", count: 32))
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("expectedDestinationBindingHash"))
        }
    }

    func testTonRouteCanaryEvidenceBindsLiveAccountSnapshot() throws {
        let evidence = try Self.sampleTonRouteCanaryEvidence()

        XCTAssertEqual(try canonicalTonSccpRouteCanaryEvidenceBytes(evidence).count, 358)
        XCTAssertEqual(
            try tonSccpRouteCanaryEvidenceHash(evidence),
            "0xf128e8405017b9ca7733bb10d43eeaf783e38d39740a3455aa353c76655c6942"
        )

        XCTAssertThrowsError(try tonSccpRouteCanaryEvidenceHash(
            try Self.sampleTonRouteCanaryEvidence(destinationBindingHash: "0x" + String(repeating: "78", count: 32))
        )) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("destinationBindingHash"))
        }
        XCTAssertThrowsError(try tonSccpRouteCanaryEvidenceHash(
            try Self.sampleTonRouteCanaryEvidence(verifierContractAddress: "1:" + String(repeating: "11", count: 32))
        )) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("verifierContractAddress"))
        }
        XCTAssertThrowsError(try tonSccpRouteCanaryEvidenceHash(
            try Self.sampleTonRouteCanaryEvidence(accountStatus: "uninit")
        )) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("accountStatus"))
        }
        XCTAssertThrowsError(try tonSccpRouteCanaryEvidenceHash(
            try Self.sampleTonRouteCanaryEvidence(lastTransactionLt: "0123")
        )) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("lastTransactionLt"))
        }
        XCTAssertThrowsError(try tonSccpRouteCanaryEvidenceHash(
            try Self.sampleTonRouteCanaryEvidence(verifierCodeBocRootHash: "0x" + String(repeating: "45", count: 32))
        )) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("verifierCodeBocRootHash"))
        }
    }

    func testTronRouteCanaryEvidenceBindsTransactionTranscript() throws {
        let evidence = try Self.sampleTronRouteCanaryEvidence()

        XCTAssertEqual(try canonicalTronSccpRouteCanaryEvidenceBytes(evidence).count, 551)
        XCTAssertEqual(
            try tronSccpRouteCanaryEvidenceHash(evidence),
            "0xe0a96ff7e8f523599fd60fffe8bb3b9fda9519126b7ba00c89c922b323b64e56"
        )

        XCTAssertThrowsError(try tronSccpRouteCanaryEvidenceHash(
            try Self.sampleTronRouteCanaryEvidence(routeAllowlistHash: "0x" + String(repeating: "78", count: 32))
        )) { error in
            XCTAssertEqual(error as? TronSccpProverError, .invalidPublicInputs("routeAllowlistHash"))
        }
        XCTAssertThrowsError(try tronSccpRouteCanaryEvidenceHash(
            try Self.sampleTronRouteCanaryEvidence(destinationBindingHash: "0x" + String(repeating: "78", count: 32))
        )) { error in
            XCTAssertEqual(error as? TronSccpProverError, .invalidPublicInputs("destinationBindingHash"))
        }
        XCTAssertThrowsError(try tronSccpRouteCanaryEvidenceHash(
            try Self.sampleTronRouteCanaryEvidence(blockNumber: 0)
        )) { error in
            XCTAssertEqual(error as? TronSccpProverError, .invalidPublicInputs("blockNumber"))
        }
        XCTAssertThrowsError(try tronSccpRouteCanaryEvidenceHash(
            try Self.sampleTronRouteCanaryEvidence(usedMessageProof: false)
        )) { error in
            XCTAssertEqual(error as? TronSccpProverError, .invalidPublicInputs("usedMessageProof"))
        }
        XCTAssertThrowsError(try tronSccpRouteCanaryEvidenceHash(
            try Self.sampleTronRouteCanaryEvidence(rawDataOwnerMatchesTransaction: false)
        )) { error in
            XCTAssertEqual(error as? TronSccpProverError, .invalidPublicInputs("rawDataOwnerMatchesTransaction"))
        }
        XCTAssertThrowsError(try tronSccpRouteCanaryEvidenceHash(
            try Self.sampleTronRouteCanaryEvidence(signatureRecoversToOwner: false)
        )) { error in
            XCTAssertEqual(error as? TronSccpProverError, .invalidPublicInputs("signatureRecoversToOwner"))
        }
        XCTAssertThrowsError(try tronSccpRouteCanaryEvidenceHash(
            try Self.sampleTronRouteCanaryEvidence(signatureRecoveredAddress: "0x41" + String(repeating: "12", count: 20))
        )) { error in
            XCTAssertEqual(error as? TronSccpProverError, .invalidPublicInputs("signatureRecoveredAddress"))
        }
        XCTAssertThrowsError(try tronSccpRouteCanaryEvidenceHash(
            try Self.sampleTronRouteCanaryEvidence(routeCanaryEvidenceHash: "0x" + String(repeating: "78", count: 32))
        )) { error in
            XCTAssertEqual(error as? TronSccpProverError, .invalidPublicInputs("routeCanaryEvidenceHash"))
        }
    }

    private final class EvmSourceProofWitnessProvider: EvmSccpWitnessProvider {
        private(set) var resolveCount = 0

        func resolveWitness(_ input: EvmSccpProofRequestInput) async throws -> EvmSccpProofRequestInput {
            XCTAssertTrue(input.sourceProofBytes.isEmpty)
            resolveCount += 1
            return EvmSccpProofRequestInput(
                publicInputs: input.publicInputs,
                bundleBytes: input.bundleBytes,
                sourceProofBytes: Data([9, 10]),
                statementHash: input.statementHash,
                destinationBindingHash: input.destinationBindingHash,
                backend: input.backend,
                sourceDomain: input.sourceDomain,
                destinationBinding: input.destinationBinding
            )
        }
    }

    private final class EthereumMainnetExecutionProviderStub: EthereumMainnetExecutionProvider {
        private let chainId: Any
        private let receipt: [String: Any]
        private let block: [String: Any]
        private let blockReceipts: [[String: Any]]?
        private(set) var calls: [String] = []

        init(chainId: Any = "0x1",
             receipt: [String: Any],
             block: [String: Any],
             blockReceipts: [[String: Any]]? = nil) {
            self.chainId = chainId
            self.receipt = receipt
            self.block = block
            self.blockReceipts = blockReceipts
        }

        func request(method: String, params: [Any]) async throws -> Any {
            calls.append(method)
            switch method {
            case "eth_chainId":
                return chainId
            case "eth_getTransactionReceipt":
                XCTAssertEqual(params.count, 1)
                XCTAssertEqual(params.first as? String, receipt["transactionHash"] as? String)
                return receipt
            case "eth_getBlockByHash":
                XCTAssertEqual(params.count, 2)
                XCTAssertEqual(params.first as? String, block["hash"] as? String)
                XCTAssertEqual(params.last as? Bool, false)
                return block
            case "eth_getBlockReceipts":
                XCTAssertEqual(params.count, 1)
                XCTAssertEqual(params.first as? String, block["number"] as? String)
                guard let blockReceipts else {
                    XCTFail("blockReceipts fixture is not configured")
                    return []
                }
                return blockReceipts
            default:
                XCTFail("unexpected Ethereum JSON-RPC method \(method)")
                return [:]
            }
        }
    }

    private final class EthereumMainnetConsensusProviderStub: EthereumMainnetConsensusProvider {
        private let finality: [String: Any]
        private(set) var calls = 0

        init(finality: [String: Any]) {
            self.finality = finality
        }

        func collectFinalityEvidence(
            receipt: [String: Any]?,
            block: [String: Any]?,
            transactionHash: String?
        ) async throws -> [String: Any] {
            calls += 1
            XCTAssertEqual(receipt?["blockHash"] as? String, block?["hash"] as? String)
            XCTAssertEqual(transactionHash, receipt?["transactionHash"] as? String)
            return finality
        }
    }

    private final class EthereumMainnetMutatingConsensusProviderStub: EthereumMainnetConsensusProvider {
        private let finality: [String: Any]
        private let onCollect: ([String: Any]?, [String: Any]?, String?) throws -> Void
        private(set) var calls = 0

        init(finality: [String: Any],
             onCollect: @escaping ([String: Any]?, [String: Any]?, String?) throws -> Void) {
            self.finality = finality
            self.onCollect = onCollect
        }

        func collectFinalityEvidence(
            receipt: [String: Any]?,
            block: [String: Any]?,
            transactionHash: String?
        ) async throws -> [String: Any] {
            calls += 1
            try onCollect(receipt, block, transactionHash)
            return finality
        }
    }

    private final class EthereumMainnetBeaconRestTransportStub: EthereumMainnetBeaconRestTransport {
        private let responses: [String: EthereumMainnetBeaconRestResponse]
        private(set) var calls: [(url: String, headers: [String: String])] = []

        init(responses: [String: EthereumMainnetBeaconRestResponse]) {
            self.responses = responses
        }

        func get(url: URL, headers: [String: String]) async throws -> EthereumMainnetBeaconRestResponse {
            calls.append((url.absoluteString, headers))
            guard let response = responses[url.absoluteString] else {
                XCTFail("unexpected Beacon REST URL \(url.absoluteString)")
                throw EvmSccpProverError.invalidPublicInputs("beaconRest.response")
            }
            return response
        }
    }

    private final class EthereumMainnetBeaconRestURLProtocol: URLProtocol {
        static var response: (statusCode: Int, headers: [String: String], body: Data)?

        override class func canInit(with request: URLRequest) -> Bool {
            true
        }

        override class func canonicalRequest(for request: URLRequest) -> URLRequest {
            request
        }

        override func startLoading() {
            guard let response = Self.response,
                  let url = request.url,
                  let http = HTTPURLResponse(
                    url: url,
                    statusCode: response.statusCode,
                    httpVersion: nil,
                    headerFields: response.headers
                  ) else {
                client?.urlProtocol(self, didFailWithError: EvmSccpProverError.invalidPublicInputs("beaconRest.response"))
                return
            }
            client?.urlProtocol(self, didReceive: http, cacheStoragePolicy: .notAllowed)
            client?.urlProtocol(self, didLoad: response.body)
            client?.urlProtocolDidFinishLoading(self)
        }

        override func stopLoading() {}
    }

    private static func ethereumBeaconHeaderJson(finalizedHeaderRoot: String = "0xed5b18104f470370f9f7ce3e5c2f4892ab541f2991e626578b76cf34819def1b",
                                                 slot: String = "32",
                                                 executionOptimistic: Bool = false,
                                                 finalized: Bool = true,
                                                 canonical: Bool = true) -> Data {
        Data("""
        {
          "execution_optimistic": \(executionOptimistic),
          "finalized": \(finalized),
          "data": {
            "root": "\(finalizedHeaderRoot)",
            "canonical": \(canonical),
              "header": {
                "message": {
                  "slot": "\(slot)",
                  "proposer_index": "1",
                  "parent_root": "0x\(String(repeating: "01", count: 32))",
                  "state_root": "0x\(String(repeating: "02", count: 32))",
                  "body_root": "0x\(String(repeating: "03", count: 32))"
                },
                "signature": "0x\(String(repeating: "12", count: 96))"
              }
            }
          }
        """.utf8)
    }

    private static func ethereumBeaconCheckpointJson(finalizedHeaderRoot: String = "0xed5b18104f470370f9f7ce3e5c2f4892ab541f2991e626578b76cf34819def1b",
                                                     executionOptimistic: Bool = false,
                                                     finalized: Bool = true) -> Data {
        Data("""
        {
          "execution_optimistic": \(executionOptimistic),
          "finalized": \(finalized),
          "data": {
            "finalized": {
              "root": "\(finalizedHeaderRoot)"
            }
          }
        }
        """.utf8)
    }

    private static func ethereumBeaconBlockRootJson(finalizedHeaderRoot: String = "0xed5b18104f470370f9f7ce3e5c2f4892ab541f2991e626578b76cf34819def1b",
                                                    executionOptimistic: Bool = false,
                                                    finalized: Bool = true) -> Data {
        Data("""
        {
          "execution_optimistic": \(executionOptimistic),
          "finalized": \(finalized),
          "data": {
            "root": "\(finalizedHeaderRoot)"
          }
        }
        """.utf8)
    }

    private static func ethereumBeaconBlockJson(slot: String = "32",
                                                blockHash: String = "0x" + String(repeating: "bb", count: 32),
                                                blockNumber: String = "4660",
                                                receiptsRoot: String = "0x" + String(repeating: "cc", count: 32),
                                                executionOptimistic: Bool = false,
                                                finalized: Bool = true) -> Data {
        Data("""
        {
          "execution_optimistic": \(executionOptimistic),
          "finalized": \(finalized),
          "data": {
            "message": {
              "slot": "\(slot)",
              "body": {
                "execution_payload": {
                  "block_hash": "\(blockHash)",
                  "block_number": "\(blockNumber)",
                  "receipts_root": "\(receiptsRoot)"
                }
              }
            }
          }
        }
        """.utf8)
    }

    private static func ethereumBeaconGenesisJson(genesisTime: String = "100") -> Data {
        Data("""
        {
          "data": {
            "genesis_time": "\(genesisTime)",
            "genesis_validators_root": "0x\(String(repeating: "ab", count: 32))",
            "genesis_fork_version": "0x00000000"
          }
        }
        """.utf8)
    }

    private static func ethereumBeaconFinalityUpdateJson(slot: String = "32",
                                                         signatureSlot: String = "33",
                                                         syncCommitteeBits: String = "0x" + String(repeating: "ff", count: 42) + "3f" + String(repeating: "00", count: 21),
                                                         syncCommitteeSignature: String = "0x" + String(repeating: "34", count: 96),
                                                         executionOptimistic: Bool = false,
                                                         includeFinalityBranch: Bool = true,
                                                         finalityBranch: [String]? = nil) -> Data {
        let finalityBranchJson = (finalityBranch ?? Self.ethereumFinalityBranch)
            .map { "\"\($0)\"" }
            .joined(separator: ",")
        let finalityBranchField: String
        if includeFinalityBranch {
            finalityBranchField = "\"finality_branch\": [\(finalityBranchJson)],"
        } else {
            finalityBranchField = ""
        }
        return Data("""
        {
          "execution_optimistic": \(executionOptimistic),
          "data": {
            "finalized_header": {
              "beacon": {
                "slot": "\(slot)",
                "proposer_index": "1",
                "parent_root": "0x\(String(repeating: "01", count: 32))",
                "state_root": "0x\(String(repeating: "02", count: 32))",
                "body_root": "0x\(String(repeating: "03", count: 32))"
              }
            },
            \(finalityBranchField)
            "sync_aggregate": {
              "sync_committee_bits": "\(syncCommitteeBits)",
              "sync_committee_signature": "\(syncCommitteeSignature)"
            },
            "signature_slot": "\(signatureSlot)"
          }
        }
        """.utf8)
    }

    private static func ethereumBeaconResponse(_ body: Data, statusCode: Int = 200) -> EthereumMainnetBeaconRestResponse {
        EthereumMainnetBeaconRestResponse(statusCode: statusCode, body: body)
    }

    private final class BscMainnetExecutionProviderStub: BscMainnetExecutionProvider {
        private let chainId: Any
        private let receipt: [String: Any]
        private let block: [String: Any]
        private(set) var calls: [String] = []

        init(chainId: Any = "0x38", receipt: [String: Any], block: [String: Any]) {
            self.chainId = chainId
            self.receipt = receipt
            self.block = block
        }

        func request(method: String, params: [Any]) async throws -> Any {
            calls.append(method)
            switch method {
            case "eth_chainId":
                return chainId
            case "eth_getTransactionReceipt":
                XCTAssertEqual(params.count, 1)
                XCTAssertEqual(params.first as? String, receipt["transactionHash"] as? String)
                return receipt
            case "eth_getBlockByHash":
                XCTAssertEqual(params.count, 2)
                XCTAssertEqual(params.first as? String, block["hash"] as? String)
                XCTAssertEqual(params.last as? Bool, false)
                return block
            default:
                XCTFail("unexpected BSC JSON-RPC method \(method)")
                return [:]
            }
        }
    }

    private final class BscMainnetConsensusProviderStub: BscMainnetConsensusProvider {
        private let finality: [String: Any]
        private(set) var calls = 0

        init(finality: [String: Any]) {
            self.finality = finality
        }

        func collectFinalityEvidence(
            receipt: [String: Any]?,
            block: [String: Any]?,
            transactionHash: String?
        ) async throws -> [String: Any] {
            calls += 1
            XCTAssertEqual(receipt?["blockHash"] as? String, block?["hash"] as? String)
            XCTAssertEqual(transactionHash, receipt?["transactionHash"] as? String)
            return finality
        }
    }

    private final class BscMainnetMutatingConsensusProviderStub: BscMainnetConsensusProvider {
        private let finality: [String: Any]
        private let onCollect: ([String: Any]?, [String: Any]?, String?) throws -> Void
        private(set) var calls = 0

        init(finality: [String: Any],
             onCollect: @escaping ([String: Any]?, [String: Any]?, String?) throws -> Void) {
            self.finality = finality
            self.onCollect = onCollect
        }

        func collectFinalityEvidence(
            receipt: [String: Any]?,
            block: [String: Any]?,
            transactionHash: String?
        ) async throws -> [String: Any] {
            calls += 1
            try onCollect(receipt, block, transactionHash)
            return finality
        }
    }

    private final class TronSourceProofWitnessProvider: TronSccpWitnessProvider {
        private(set) var resolveCount = 0

        func resolveWitness(_ input: TronSccpProofRequestInput) async throws -> TronSccpProofRequestInput {
            XCTAssertTrue(input.sourceProofBytes.isEmpty)
            resolveCount += 1
            return TronSccpProofRequestInput(
                publicInputs: input.publicInputs,
                bundleBytes: input.bundleBytes,
                sourceProofBytes: Data([9, 10]),
                statementHash: input.statementHash,
                destinationBindingHash: input.destinationBindingHash,
                backend: input.backend,
                sourceDomain: input.sourceDomain,
                destinationBinding: input.destinationBinding
            )
        }
    }

    private final class TonSourceProofWitnessProvider: TonSccpWitnessProvider {
        private(set) var resolveCount = 0

        func resolveWitness(_ input: TonSccpProofRequestInput) async throws -> TonSccpProofRequestInput {
            XCTAssertTrue(input.sourceProofBytes.isEmpty)
            resolveCount += 1
            return TonSccpProofRequestInput(
                publicInputs: input.publicInputs,
                bundleBytes: input.bundleBytes,
                sourceProofBytes: Data([9, 10]),
                statementHash: input.statementHash,
                destinationBindingHash: input.destinationBindingHash,
                sourceStateVerifierId: input.sourceStateVerifierId,
                sourceStateVerifierHash: input.sourceStateVerifierHash,
                sourceAdapterDeploymentHash: input.sourceAdapterDeploymentHash,
                sourceAdapterDeploymentReceiptHash: input.sourceAdapterDeploymentReceiptHash,
                backend: input.backend,
                sourceDomain: input.sourceDomain
            )
        }
    }


    private final class SolanaDestinationBindingWitnessProvider: SolanaSccpWitnessProvider {
        private let resolvedDestinationBindingHash: String
        private(set) var resolveCount = 0

        init(resolvedDestinationBindingHash: String) {
            self.resolvedDestinationBindingHash = resolvedDestinationBindingHash
        }

        func resolveWitness(_ input: SolanaSccpWitnessInput) async throws -> SolanaSccpWitnessInput {
            XCTAssertEqual(input.destinationBindingHash, String(repeating: "78", count: 32))
            resolveCount += 1
            return SolanaSccpWitnessInput(
                targetDomain: input.targetDomain,
                mainnetGenesisHash: input.mainnetGenesisHash,
                finalizedSlot: input.finalizedSlot,
                parentSlot: input.parentSlot,
                bankSignatureCount: input.bankSignatureCount,
                parentBankHash: input.parentBankHash,
                blockhash: input.blockhash,
                bankHash: input.bankHash,
                transactionStatusRoot: input.transactionStatusRoot,
                messageProofHash: input.messageProofHash,
                accountInclusionRoot: input.accountInclusionRoot,
                accountsLtHashChecksum: input.accountsLtHashChecksum,
                accountsLtHashProofPublicInputsHash: input.accountsLtHashProofPublicInputsHash,
                bankHashHardForkData: input.bankHashHardForkData,
                accountsLtHash: input.accountsLtHash,
                transactionSignature: input.transactionSignature,
                emitterProgramId: input.emitterProgramId,
                messageId: input.messageId,
                payloadHash: input.payloadHash,
                commitmentRoot: input.commitmentRoot,
                sourceEventDigest: input.sourceEventDigest,
                sourceStateVerifierId: input.sourceStateVerifierId,
                sourceStateVerifierHash: input.sourceStateVerifierHash,
                statementHash: input.statementHash,
                destinationBindingHash: resolvedDestinationBindingHash,
                inclusionBranch: input.inclusionBranch,
                sourceAdapterDeploymentHash: input.sourceAdapterDeploymentHash,
                sourceAdapterDeploymentReceiptHash: input.sourceAdapterDeploymentReceiptHash
            )
        }
    }

    private static func sampleSolanaStakeStateV2StakeAccount() -> Data {
        var data = Data(repeating: 0, count: 200)
        data[0..<4] = Data([2, 0, 0, 0])
        data[12..<44] = Data(repeating: 0x81, count: 32)
        data[44..<76] = Data(repeating: 0x91, count: 32)
        data[124..<156] = Data(repeating: 0xa1, count: 32)
        data[156..<164] = Data([0xe8, 0x03, 0, 0, 0, 0, 0, 0])
        data[164..<172] = Data([2, 0, 0, 0, 0, 0, 0, 0])
        data[172..<180] = Data([9, 0, 0, 0, 0, 0, 0, 0])
        data[180..<188] = Data([0x0a, 0xd7, 0xa3, 0x70, 0x3d, 0x0a, 0xb7, 0x3f])
        data[188..<196] = Data([123, 0, 0, 0, 0, 0, 0, 0])
        data[196] = 1
        return data
    }

    private static func sampleSolanaVoteStateAccount(hasLatency: Bool = true) -> Data {
        var data = Data(repeating: 0, count: 3_762)
        var cursor = 0

        func writeU8(_ value: UInt8) {
            data[cursor] = value
            cursor += 1
        }

        func writeU32(_ value: UInt32) {
            data[cursor..<(cursor + 4)] = Data([
                UInt8(value & 0xff),
                UInt8((value >> 8) & 0xff),
                UInt8((value >> 16) & 0xff),
                UInt8((value >> 24) & 0xff),
            ])
            cursor += 4
        }

        func writeU64(_ value: UInt64) {
            var bytes = Data()
            for shift in stride(from: 0, through: 56, by: 8) {
                bytes.append(UInt8((value >> UInt64(shift)) & 0xff))
            }
            data[cursor..<(cursor + 8)] = bytes
            cursor += 8
        }

        func writeRepeated(_ value: UInt8, count: Int) {
            data[cursor..<(cursor + count)] = Data(repeating: value, count: count)
            cursor += count
        }

        writeU32(hasLatency ? 2 : 1)
        writeRepeated(0x51, count: 32)
        writeRepeated(0x71, count: 32)
        writeU8(7)
        writeU64(sccpSolanaTowerVoteStackDepth)
        for index in 0..<Int(sccpSolanaTowerVoteStackDepth) {
            if hasLatency {
                writeU8(0)
            }
            writeU64(11 + UInt64(index))
            writeU32(UInt32(Int(sccpSolanaTowerVoteStackDepth) - index))
        }
        writeU8(1)
        writeU64(10)
        writeU64(2)
        writeU64(1)
        writeRepeated(0x60, count: 32)
        writeU64(3)
        writeRepeated(0x61, count: 32)
        return data
    }

    private static func sampleSolanaVoteStateV4Account(
        withBls: Bool = true,
        authorizedVoterCount: Int = 2
    ) -> Data {
        var data = Data(repeating: 0, count: 3_762)
        var cursor = 0

        func writeU8(_ value: UInt8) {
            data[cursor] = value
            cursor += 1
        }

        func writeU16(_ value: UInt16) {
            data[cursor..<(cursor + 2)] = Data([
                UInt8(value & 0xff),
                UInt8((value >> 8) & 0xff),
            ])
            cursor += 2
        }

        func writeU32(_ value: UInt32) {
            data[cursor..<(cursor + 4)] = Data([
                UInt8(value & 0xff),
                UInt8((value >> 8) & 0xff),
                UInt8((value >> 16) & 0xff),
                UInt8((value >> 24) & 0xff),
            ])
            cursor += 4
        }

        func writeU64(_ value: UInt64) {
            var bytes = Data()
            for shift in stride(from: 0, through: 56, by: 8) {
                bytes.append(UInt8((value >> UInt64(shift)) & 0xff))
            }
            data[cursor..<(cursor + 8)] = bytes
            cursor += 8
        }

        func writeRepeated(_ value: UInt8, count: Int) {
            data[cursor..<(cursor + count)] = Data(repeating: value, count: count)
            cursor += count
        }

        writeU32(3)
        writeRepeated(0x51, count: 32)
        writeRepeated(0x71, count: 32)
        writeRepeated(0x81, count: 32)
        writeRepeated(0x91, count: 32)
        writeU16(1_234)
        writeU16(9_876)
        writeU64(456)
        writeU8(withBls ? 1 : 0)
        if withBls {
            writeRepeated(0xa5, count: 48)
        }
        writeU64(sccpSolanaTowerVoteStackDepth)
        for index in 0..<Int(sccpSolanaTowerVoteStackDepth) {
            writeU8(0)
            writeU64(11 + UInt64(index))
            writeU32(UInt32(Int(sccpSolanaTowerVoteStackDepth) - index))
        }
        writeU8(1)
        writeU64(10)
        writeU64(UInt64(authorizedVoterCount))
        for index in 0..<authorizedVoterCount {
            writeU64(UInt64(index + 1))
            writeRepeated(0x60 + UInt8(index), count: 32)
        }
        return data
    }

    func testBuildsSolanaSccpProofRequest() throws {
        let request = try buildSolanaSccpProofRequest(Self.sampleWitness())

        XCTAssertEqual(request.version, 1)
        XCTAssertEqual(request.backend, sccpSolanaRecursiveProofBackendV1)
        XCTAssertEqual(request.sourceDomain, sccpDomainSolana)
        XCTAssertEqual(request.targetDomain, sccpDomainSora)
        XCTAssertEqual(request.mainnetGenesisHash, sccpSolanaMainnetGenesisHash)
        XCTAssertTrue(request.witness.blockhash.hasPrefix("0x"))
        XCTAssertEqual(request.witness.blockhash.count, 66)
        XCTAssertEqual(
            request.witnessHash,
            try buildSolanaSccpProofRequest(Self.sampleWitness(blockhash: request.witness.blockhash)).witnessHash
        )
        XCTAssertEqual(request.publicInputs.messageId, "0x" + String(repeating: "dd", count: 32))
        XCTAssertEqual(request.publicInputs.bankHash, "0x" + String(repeating: "aa", count: 32))
        XCTAssertEqual(request.publicInputs.parentSlot, 320)
        XCTAssertEqual(request.publicInputs.bankSignatureCount, 8)
        XCTAssertEqual(
            request.publicInputs.accountsLtHashProofPublicInputsHash,
            try solanaSccpAccountsLtHashProofPublicInputsHash(
                finalizedSlot: request.witness.finalizedSlot,
                parentSlot: request.witness.parentSlot,
                bankSignatureCount: request.witness.bankSignatureCount,
                parentBankHash: request.witness.parentBankHash,
                bankHash: request.witness.bankHash,
                blockhash: request.witness.blockhash,
                bankHashHardForkData: request.witness.bankHashHardForkData,
                transactionStatusRoot: request.witness.transactionStatusRoot,
                accountInclusionRoot: request.witness.accountInclusionRoot,
                accountsLtHashChecksum: request.witness.accountsLtHashChecksum
            )
        )
        XCTAssertEqual(
            request.publicInputs.transactionStatusRoot,
            "0x" + String(repeating: "bb", count: 32)
        )
        XCTAssertEqual(request.publicInputs.messageProofHash, "0x" + String(repeating: "cc", count: 32))
        XCTAssertEqual(request.publicInputs.statementHash, "0x" + String(repeating: "56", count: 32))
        XCTAssertEqual(request.publicInputs.destinationBindingHash, "0x" + String(repeating: "78", count: 32))
        XCTAssertEqual(request.sourceStateVerifierId, sccpSolanaMainnetAccountsDbVerifierIdV1)
        XCTAssertEqual(request.sourceStateVerifierHash, sccpZeroHashV1)
        XCTAssertEqual(request.publicInputs.sourceStateVerifierId, sccpSolanaMainnetAccountsDbVerifierIdV1)
        XCTAssertEqual(request.publicInputs.sourceStateVerifierHash, sccpZeroHashV1)
        XCTAssertEqual(request.publicInputs.sourceAdapterDeploymentHash, sccpZeroHashV1)
        XCTAssertEqual(request.publicInputs.sourceAdapterDeploymentReceiptHash, sccpZeroHashV1)
        XCTAssertEqual(
            request.sourceAdapterDeploymentBindingHash,
            try sccpSourceAdapterDeploymentBindingHash(request.sourceAdapterDeploymentBinding)
        )
        XCTAssertEqual(request.proofContext.statementHash, request.publicInputs.statementHash)
        XCTAssertTrue(request.proofContextHash.hasPrefix("0x"))
        XCTAssertEqual(request.proofContextHash.count, 66)
        XCTAssertEqual(request.sourceAdapterDeploymentBindingHash.count, 66)
        XCTAssertGreaterThan(try canonicalSolanaSccpProofContextBytes(request.proofContext).count, 0)
        XCTAssertGreaterThan(
            try canonicalSolanaSccpAccountsLtHashProofPublicInputsBytes(
                finalizedSlot: request.witness.finalizedSlot,
                parentSlot: request.witness.parentSlot,
                bankSignatureCount: request.witness.bankSignatureCount,
                parentBankHash: request.witness.parentBankHash,
                bankHash: request.witness.bankHash,
                blockhash: request.witness.blockhash,
                bankHashHardForkData: request.witness.bankHashHardForkData,
                transactionStatusRoot: request.witness.transactionStatusRoot,
                accountInclusionRoot: request.witness.accountInclusionRoot,
                accountsLtHashChecksum: request.witness.accountsLtHashChecksum
            ).count,
            250
        )
        XCTAssertTrue(request.witnessHash.hasPrefix("0x"))
        XCTAssertEqual(request.witnessHash.count, 66)
    }

    func testSolanaProofRequestRequiresSoraTargetDomain() {
        XCTAssertThrowsError(try buildSolanaSccpProofRequest(Self.sampleWitness(
            targetDomain: sccpDomainTon
        ))) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("targetDomain"))
        }
    }

    func testRequiresSourceEventDigest() {
        let witness = SolanaSccpWitnessInput(
            finalizedSlot: 321,
            parentSlot: 320,
            bankSignatureCount: 8,
            parentBankHash: String(repeating: "c0", count: 32),
            blockhash: "9xQeWvG816bUx9EPfYdLSdJH7Gq2Xv3yQPG8mD3kAcL7",
            bankHash: String(repeating: "aa", count: 32),
            transactionStatusRoot: String(repeating: "bb", count: 32),
            messageProofHash: String(repeating: "cc", count: 32),
            accountInclusionRoot: String(repeating: "77", count: 32),
            accountsLtHashChecksum: String(repeating: "88", count: 32),
            transactionSignature: Self.solanaSignature55,
            emitterProgramId: Self.solanaProgram42,
            messageId: String(repeating: "dd", count: 32),
            payloadHash: String(repeating: "ee", count: 32),
            commitmentRoot: String(repeating: "12", count: 32),
            sourceEventDigest: "",
            statementHash: String(repeating: "56", count: 32),
            destinationBindingHash: String(repeating: "78", count: 32)
        )

        XCTAssertThrowsError(try normalizeSolanaSccpWitness(witness)) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidHex32("sourceEventDigest"))
        }
    }

    func testBuildsSolanaMessageProofHashFromInclusionWitness() throws {
        let branch = [Data(repeating: 0x56, count: 32)]
        let transactionStatusRoot = try solanaSccpTransactionStatusRootFromBranch(
            sourceEventDigest: String(repeating: "34", count: 32),
            transactionSignature: Self.solanaSignature55,
            emitterProgramId: Self.solanaProgram42,
            inclusionBranch: branch
        )
        XCTAssertEqual(
            transactionStatusRoot,
            "0xb048ca31d8ad7b2a0d15cbeb81d536350743483d44dd93136e859df93d3863b2"
        )
        let hash = try solanaSccpMessageProofHash(
            sourceEventDigest: String(repeating: "34", count: 32),
            transactionStatusRoot: transactionStatusRoot,
            transactionSignature: Self.solanaSignature55,
            emitterProgramId: Self.solanaProgram42,
            inclusionBranch: branch
        )

        XCTAssertTrue(hash.hasPrefix("0x"))
        XCTAssertEqual(hash.count, 66)
        XCTAssertGreaterThan(
            try canonicalSolanaSccpTransactionStatusLeafBytes(
                sourceEventDigest: String(repeating: "34", count: 32),
                transactionSignature: Self.solanaSignature55,
                emitterProgramId: Self.solanaProgram42
            ).count,
            0
        )
        XCTAssertEqual(
            try solanaSccpTransactionStatusLeafHash(
                sourceEventDigest: String(repeating: "34", count: 32),
                transactionSignature: Self.solanaSignature55,
                emitterProgramId: Self.solanaProgram42
            ),
            "0x4e12efed6d53466de0596f05aa6cc767df1efd6a4d1549276c4ec8b69118515d"
        )
        XCTAssertThrowsError(try solanaSccpTransactionStatusLeafHash(
            sourceEventDigest: String(repeating: "34", count: 32),
            transactionSignature: Self.solanaZeroSignature,
            emitterProgramId: Self.solanaProgram42
        ))
        XCTAssertThrowsError(try solanaSccpTransactionStatusLeafHash(
            sourceEventDigest: String(repeating: "34", count: 32),
            transactionSignature: Self.solanaSignature55,
            emitterProgramId: Self.solanaZeroProgram
        ))
        XCTAssertGreaterThan(
            try canonicalSolanaSccpMessageProofBytes(
                sourceEventDigest: String(repeating: "34", count: 32),
                transactionStatusRoot: transactionStatusRoot,
                transactionSignature: Self.solanaSignature55,
                emitterProgramId: Self.solanaProgram42,
                inclusionBranch: branch
            ).count,
            0
        )
        XCTAssertNotEqual(
            hash,
            try solanaSccpMessageProofHash(
                sourceEventDigest: String(repeating: "34", count: 32),
                transactionStatusRoot: transactionStatusRoot,
                transactionSignature: Self.solanaSignature01,
                emitterProgramId: Self.solanaProgram42,
                inclusionBranch: branch
            )
        )
        XCTAssertThrowsError(try solanaSccpMessageProofHash(
            sourceEventDigest: String(repeating: "34", count: 32),
            transactionStatusRoot: transactionStatusRoot,
            transactionSignature: Self.solanaZeroSignature,
            emitterProgramId: Self.solanaProgram42,
            inclusionBranch: branch
        ))
        XCTAssertThrowsError(try solanaSccpMessageProofHash(
            sourceEventDigest: String(repeating: "34", count: 32),
            transactionStatusRoot: transactionStatusRoot,
            transactionSignature: Self.solanaSignature55,
            emitterProgramId: Self.solanaZeroProgram,
            inclusionBranch: branch
        ))
        XCTAssertNotEqual(
            hash,
            try solanaSccpMessageProofHash(
                sourceEventDigest: String(repeating: "34", count: 32),
                transactionStatusRoot: transactionStatusRoot,
                transactionSignature: Self.solanaSignature55,
                emitterProgramId: Self.solanaProgram02,
                inclusionBranch: branch
            )
        )
        XCTAssertThrowsError(
            try solanaSccpMessageProofHash(
                sourceEventDigest: String(repeating: "34", count: 32),
                transactionStatusRoot: transactionStatusRoot,
                transactionSignature: Self.solanaSignature55,
                emitterProgramId: Self.solanaProgram42,
                inclusionBranch: []
            )
        )
        XCTAssertThrowsError(
            try solanaSccpMessageProofHash(
                sourceEventDigest: String(repeating: "34", count: 32),
                transactionStatusRoot: transactionStatusRoot,
                transactionSignature: Self.solanaSignature55,
                emitterProgramId: Self.solanaProgram42,
                inclusionBranch: Array(
                    repeating: Data(repeating: 0x56, count: 32),
                    count: sccpMaxSourceMerkleBranchNodes + 1
                )
            )
        ) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("inclusionBranch"))
        }
        XCTAssertThrowsError(
            try solanaSccpMessageProofHash(
                sourceEventDigest: String(repeating: "34", count: 32),
                transactionStatusRoot: transactionStatusRoot,
                transactionSignature: Self.solanaSignature55,
                emitterProgramId: Self.solanaProgram42,
                inclusionBranch: [Data(repeating: 0xab, count: 31)]
            )
        )
        XCTAssertThrowsError(
            try solanaSccpMessageProofHash(
                sourceEventDigest: String(repeating: "34", count: 32),
                transactionStatusRoot: transactionStatusRoot,
                transactionSignature: "not-a-solana-signature",
                emitterProgramId: Self.solanaProgram42,
                inclusionBranch: branch
            )
        )
    }

    func testBuildsSolanaEpochStakeRootForVoteWitnesses() throws {
        let validatorPublicKeys = [
            Data(repeating: 0x11, count: 32),
            Data(repeating: 0x22, count: 32),
        ]
        let validatorStakes: [UInt64] = [1, 2]

        XCTAssertEqual(sccpSolanaMainnetSlotsPerEpoch, 432_000)
        XCTAssertEqual(solanaSccpMainnetEpoch(forSlot: 864_000), 2)
        XCTAssertEqual(
            try canonicalSolanaSccpEpochStakeRootBytes(
                epoch: 3,
                validatorPublicKeys: validatorPublicKeys,
                validatorStakes: validatorStakes
            ).count,
            134
        )
        XCTAssertEqual(
            try solanaSccpEpochStakeRoot(
                epoch: 3,
                validatorPublicKeys: validatorPublicKeys,
                validatorStakes: validatorStakes
            ),
            "0x1d86a5ecfac6e63bfcefdc1a3bfefd962a33e2a4cf65cd4e8518bcebea771f0a"
        )
        XCTAssertThrowsError(try solanaSccpEpochStakeRoot(
            epoch: 3,
            validatorPublicKeys: [Data(repeating: 0x11, count: 31)],
            validatorStakes: [1]
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidHex32("validatorPublicKeys[0]"))
        }
        XCTAssertThrowsError(try solanaSccpEpochStakeRoot(
            epoch: 3,
            validatorPublicKeys: [Data(repeating: 0x00, count: 32)],
            validatorStakes: [1]
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("validatorPublicKeys[0]"))
        }
        let oversizedValidatorPublicKeys = (0...sccpSolanaMaxValidators).map { index -> Data in
            var publicKey = Data(repeating: 0x00, count: 32)
            var encodedIndex = UInt64(index + 1).littleEndian
            withUnsafeBytes(of: &encodedIndex) { publicKey.replaceSubrange(24..<32, with: $0) }
            return publicKey
        }
        XCTAssertThrowsError(try solanaSccpEpochStakeRoot(
            epoch: 3,
            validatorPublicKeys: oversizedValidatorPublicKeys,
            validatorStakes: Array(repeating: 1, count: oversizedValidatorPublicKeys.count)
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("validatorPublicKeys"))
        }
    }

    func testBuildsSolanaStakeActivationHashForFinalityContext() throws {
        let validatorPublicKeys = [
            Data(repeating: 0x11, count: 32),
            Data(repeating: 0x22, count: 32),
        ]
        let validatorStakes: [UInt64] = [1, 2]
        let activationEpochs: [UInt64] = [0, 2]
        let deactivationEpochs: [UInt64] = [UInt64.max, 9]

        XCTAssertEqual(
            try canonicalSolanaSccpStakeActivationBytes(
                epoch: 3,
                validatorPublicKeys: validatorPublicKeys,
                validatorStakes: validatorStakes,
                validatorActivationEpochs: activationEpochs,
                validatorDeactivationEpochs: deactivationEpochs
            ).count,
            165
        )
        XCTAssertEqual(
            try solanaSccpStakeActivationHash(
                epoch: 3,
                validatorPublicKeys: validatorPublicKeys,
                validatorStakes: validatorStakes,
                validatorActivationEpochs: activationEpochs,
                validatorDeactivationEpochs: deactivationEpochs
            ),
            "0xdb418c62a1aeb8ae15cb26e3a198d46890cefa3545df8e1921be2e83f57dabf3"
        )
        XCTAssertThrowsError(try solanaSccpStakeActivationHash(
            epoch: 3,
            validatorPublicKeys: validatorPublicKeys,
            validatorStakes: validatorStakes,
            validatorActivationEpochs: [4, 2],
            validatorDeactivationEpochs: deactivationEpochs
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("validatorActivationEpochs[0]"))
        }
        XCTAssertThrowsError(try solanaSccpStakeActivationHash(
            epoch: 3,
            validatorPublicKeys: validatorPublicKeys,
            validatorStakes: validatorStakes,
            validatorActivationEpochs: [3, 2],
            validatorDeactivationEpochs: deactivationEpochs
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("validatorActivationEpochs[0]"))
        }
        XCTAssertThrowsError(try solanaSccpStakeActivationHash(
            epoch: 3,
            validatorPublicKeys: validatorPublicKeys,
            validatorStakes: validatorStakes,
            validatorActivationEpochs: activationEpochs,
            validatorDeactivationEpochs: [UInt64.max, 2]
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("validatorDeactivationEpochs[1]"))
        }
        XCTAssertEqual(
            try solanaSccpStakeActivationHash(
                epoch: 3,
                validatorPublicKeys: validatorPublicKeys,
                validatorStakes: validatorStakes,
                validatorActivationEpochs: activationEpochs,
                validatorDeactivationEpochs: [UInt64.max, 3]
            ).count,
            66
        )
        XCTAssertThrowsError(try solanaSccpStakeActivationHash(
            epoch: 3,
            validatorPublicKeys: validatorPublicKeys,
            validatorStakes: validatorStakes,
            validatorActivationEpochs: [0],
            validatorDeactivationEpochs: deactivationEpochs
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("validatorActivationEpochs"))
        }
    }

    func testBuildsSolanaAccountOpeningHashForFinalityContext() throws {
        let address = Data(repeating: 0x31, count: 32)
        let dataHash = Data(repeating: 0x71, count: 32)

        XCTAssertEqual(
            try canonicalSolanaSccpAccountOpeningBytes(
                address: address,
                owner: sccpSolanaVoteProgramId,
                lamports: 1_000_000,
                rentEpoch: 0,
                executable: false,
                dataHash: dataHash
            ).count,
            122
        )
        let accountHash = try solanaSccpAccountOpeningHash(
            address: address,
            owner: sccpSolanaVoteProgramId,
            lamports: 1_000_000,
            rentEpoch: 0,
            executable: false,
            dataHash: dataHash
        )
        XCTAssertTrue(accountHash.range(of: #"^0x[0-9a-f]{64}$"#, options: .regularExpression) != nil)
        XCTAssertNotEqual(
            accountHash,
            try solanaSccpAccountOpeningHash(
                address: address,
                owner: sccpSolanaStakeProgramId,
                lamports: 1_000_000,
                rentEpoch: 0,
                executable: false,
                dataHash: dataHash
            )
        )
        XCTAssertNotEqual(
            accountHash,
            try solanaSccpAccountOpeningHash(
                address: address,
                owner: sccpSolanaVoteProgramId,
                lamports: 1_000_000,
                rentEpoch: 0,
                executable: true,
                dataHash: dataHash
            )
        )
        XCTAssertThrowsError(try solanaSccpAccountOpeningHash(
            address: address,
            owner: sccpSolanaVoteProgramId,
            lamports: 0,
            rentEpoch: 0,
            executable: false,
            dataHash: dataHash
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("lamports"))
        }
    }

    func testBuildsOpenedAccountsLtHashContributionBindings() throws {
        func ltHash(_ value: UInt16) -> Data {
            var out = Data(repeating: 0, count: 2_048)
            for index in stride(from: 0, to: out.count, by: 2) {
                let word = value &+ UInt16(index / 2) &* 17
                out[index] = UInt8(word & 0xff)
                out[index + 1] = UInt8(word >> 8)
            }
            return out
        }
        func add(_ left: Data, _ right: Data) -> Data {
            var out = left
            for index in stride(from: 0, to: out.count, by: 2) {
                let mixed = (UInt16(out[index]) | (UInt16(out[index + 1]) << 8))
                    &+ (UInt16(right[index]) | (UInt16(right[index + 1]) << 8))
                out[index] = UInt8(mixed & 0xff)
                out[index + 1] = UInt8(mixed >> 8)
            }
            return out
        }
        let voteOpening = SolanaSccpAccountOpeningInput(
            address: Data(repeating: 0x31, count: 32),
            owner: sccpSolanaVoteProgramId,
            lamports: 1_000_000,
            rentEpoch: 0,
            dataHash: "0x" + String(repeating: "91", count: 32)
        )
        let stakeOpening = SolanaSccpAccountOpeningInput(
            address: Data(repeating: 0x32, count: 32),
            owner: sccpSolanaStakeProgramId,
            lamports: 2_000_000,
            rentEpoch: 0,
            dataHash: "0x" + String(repeating: "92", count: 32)
        )
        let stakeHistoryOpening = SolanaSccpAccountOpeningInput(
            address: sccpSolanaStakeHistorySysvarId,
            owner: sccpSolanaSysvarProgramId,
            lamports: 1,
            rentEpoch: 0,
            dataHash: "0x" + String(repeating: "93", count: 32)
        )
        let unopenedOpening = SolanaSccpAccountOpeningInput(
            address: Data(repeating: 0x34, count: 32),
            owner: sccpSolanaStakeProgramId,
            lamports: 3_000_000,
            rentEpoch: 0,
            dataHash: "0x" + String(repeating: "94", count: 32)
        )
        let voteRawData = Data([1, 2, 3])
        let stakeRawData = Data([4, 5, 6])
        let stakeHistoryRawData = Data([7, 8, 9])
        let unopenedRawData = Data([10, 11, 12])
        let voteLtHash = try solanaSccpAccountLtHash(opening: voteOpening, rawData: voteRawData)
        let stakeLtHash = try solanaSccpAccountLtHash(opening: stakeOpening, rawData: stakeRawData)
        let stakeHistoryLtHash = try solanaSccpAccountLtHash(
            opening: stakeHistoryOpening,
            rawData: stakeHistoryRawData
        )
        let openedLtHash = try solanaSccpAccountsLtHashFromOpenings(
            openings: [voteOpening, stakeOpening, stakeHistoryOpening],
            rawDataValues: [voteRawData, stakeRawData, stakeHistoryRawData]
        )
        let unopenedLtHash = try solanaSccpAccountLtHash(
            opening: unopenedOpening,
            rawData: unopenedRawData
        )
        let accountsLtHash = try solanaSccpAccountsLtHashFromOpenings(
            openings: [voteOpening, stakeOpening, stakeHistoryOpening, unopenedOpening],
            rawDataValues: [voteRawData, stakeRawData, stakeHistoryRawData, unopenedRawData]
        )
        let input = SolanaSccpOpenedAccountsLtHashContributionsInput(
            finalizedSlot: 1_296_096,
            accountInclusionRoot: "0x" + String(repeating: "77", count: 32),
            accountsLtHashChecksum: try solanaSccpAccountsLtHashChecksum(accountsLtHash),
            accountsLtHash: accountsLtHash,
            validatorVoteAccountOpenings: [voteOpening],
            validatorVoteAccountRawData: [voteRawData],
            validatorVoteAccountLtHashes: [voteLtHash],
            validatorStakeAccountOpenings: [stakeOpening],
            validatorStakeAccountRawData: [stakeRawData],
            validatorStakeAccountLtHashes: [stakeLtHash],
            stakeHistorySysvarOpening: stakeHistoryOpening,
            stakeHistorySysvarRawData: stakeHistoryRawData,
            stakeHistorySysvarAccountLtHash: stakeHistoryLtHash
        )

        XCTAssertEqual(try solanaSccpOpenedAccountsLtHashResidual(input), unopenedLtHash)
        XCTAssertEqual(
            try solanaSccpOpenedAccountsLtHashResidualChecksum(input),
            try solanaSccpAccountsLtHashChecksum(unopenedLtHash)
        )
        XCTAssertEqual(
            try canonicalSolanaSccpOpenedAccountsLtHashContributionsBytes(input).count,
            10_696
        )
        XCTAssertEqual(
            try solanaSccpOpenedAccountsLtHashContributionsHash(input),
            "0x07270072f8b70b755ed491c1582b40050a484edd67752a8a0bbbd97aa175d4f9"
        )
        let badInput = SolanaSccpOpenedAccountsLtHashContributionsInput(
            finalizedSlot: input.finalizedSlot,
            accountInclusionRoot: input.accountInclusionRoot,
            accountsLtHashChecksum: "0x" + String(repeating: "88", count: 32),
            accountsLtHash: input.accountsLtHash,
            validatorVoteAccountOpenings: input.validatorVoteAccountOpenings,
            validatorVoteAccountRawData: input.validatorVoteAccountRawData,
            validatorVoteAccountLtHashes: input.validatorVoteAccountLtHashes,
            validatorStakeAccountOpenings: input.validatorStakeAccountOpenings,
            validatorStakeAccountRawData: input.validatorStakeAccountRawData,
            validatorStakeAccountLtHashes: input.validatorStakeAccountLtHashes,
            stakeHistorySysvarOpening: input.stakeHistorySysvarOpening,
            stakeHistorySysvarRawData: input.stakeHistorySysvarRawData,
            stakeHistorySysvarAccountLtHash: input.stakeHistorySysvarAccountLtHash
        )
        XCTAssertThrowsError(try solanaSccpOpenedAccountsLtHashContributionsHash(badInput))

        let zeroResidualInput = SolanaSccpOpenedAccountsLtHashContributionsInput(
            finalizedSlot: input.finalizedSlot,
            accountInclusionRoot: input.accountInclusionRoot,
            accountsLtHashChecksum: try solanaSccpAccountsLtHashChecksum(openedLtHash),
            accountsLtHash: openedLtHash,
            validatorVoteAccountOpenings: input.validatorVoteAccountOpenings,
            validatorVoteAccountRawData: input.validatorVoteAccountRawData,
            validatorVoteAccountLtHashes: input.validatorVoteAccountLtHashes,
            validatorStakeAccountOpenings: input.validatorStakeAccountOpenings,
            validatorStakeAccountRawData: input.validatorStakeAccountRawData,
            validatorStakeAccountLtHashes: input.validatorStakeAccountLtHashes,
            stakeHistorySysvarOpening: input.stakeHistorySysvarOpening,
            stakeHistorySysvarRawData: input.stakeHistorySysvarRawData,
            stakeHistorySysvarAccountLtHash: input.stakeHistorySysvarAccountLtHash
        )
        XCTAssertThrowsError(try solanaSccpOpenedAccountsLtHashContributionsHash(zeroResidualInput)) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("openedAccountsLtHashResidual"))
        }

        let duplicateStakeOpening = SolanaSccpAccountOpeningInput(
            address: voteOpening.address,
            owner: stakeOpening.owner,
            lamports: stakeOpening.lamports,
            rentEpoch: stakeOpening.rentEpoch,
            dataHash: stakeOpening.dataHash
        )
        let duplicateInput = SolanaSccpOpenedAccountsLtHashContributionsInput(
            finalizedSlot: input.finalizedSlot,
            accountInclusionRoot: input.accountInclusionRoot,
            accountsLtHashChecksum: input.accountsLtHashChecksum,
            accountsLtHash: input.accountsLtHash,
            validatorVoteAccountOpenings: input.validatorVoteAccountOpenings,
            validatorVoteAccountRawData: input.validatorVoteAccountRawData,
            validatorVoteAccountLtHashes: input.validatorVoteAccountLtHashes,
            validatorStakeAccountOpenings: [duplicateStakeOpening],
            validatorStakeAccountRawData: input.validatorStakeAccountRawData,
            validatorStakeAccountLtHashes: input.validatorStakeAccountLtHashes,
            stakeHistorySysvarOpening: input.stakeHistorySysvarOpening,
            stakeHistorySysvarRawData: input.stakeHistorySysvarRawData,
            stakeHistorySysvarAccountLtHash: input.stakeHistorySysvarAccountLtHash
        )
        XCTAssertThrowsError(try solanaSccpOpenedAccountsLtHashContributionsHash(duplicateInput)) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("openedAccountAddresses"))
        }

        let zeroLamportsVoteOpening = SolanaSccpAccountOpeningInput(
            address: voteOpening.address,
            owner: voteOpening.owner,
            lamports: 0,
            rentEpoch: voteOpening.rentEpoch,
            dataHash: voteOpening.dataHash
        )
        let zeroLamportsInput = SolanaSccpOpenedAccountsLtHashContributionsInput(
            finalizedSlot: input.finalizedSlot,
            accountInclusionRoot: input.accountInclusionRoot,
            accountsLtHashChecksum: input.accountsLtHashChecksum,
            accountsLtHash: input.accountsLtHash,
            validatorVoteAccountOpenings: [zeroLamportsVoteOpening],
            validatorVoteAccountRawData: input.validatorVoteAccountRawData,
            validatorVoteAccountLtHashes: [Data(repeating: 0, count: 2_048)],
            validatorStakeAccountOpenings: input.validatorStakeAccountOpenings,
            validatorStakeAccountRawData: input.validatorStakeAccountRawData,
            validatorStakeAccountLtHashes: input.validatorStakeAccountLtHashes,
            stakeHistorySysvarOpening: input.stakeHistorySysvarOpening,
            stakeHistorySysvarRawData: input.stakeHistorySysvarRawData,
            stakeHistorySysvarAccountLtHash: input.stakeHistorySysvarAccountLtHash
        )
        XCTAssertThrowsError(try solanaSccpOpenedAccountsLtHashContributionsHash(zeroLamportsInput)) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("lamports"))
        }

        let oversizedVoteInput = SolanaSccpOpenedAccountsLtHashContributionsInput(
            finalizedSlot: input.finalizedSlot,
            accountInclusionRoot: input.accountInclusionRoot,
            accountsLtHashChecksum: input.accountsLtHashChecksum,
            accountsLtHash: input.accountsLtHash,
            validatorVoteAccountOpenings: Array(
                repeating: voteOpening,
                count: sccpSolanaMaxValidators + 1
            ),
            validatorVoteAccountRawData: Array(
                repeating: voteRawData,
                count: sccpSolanaMaxValidators + 1
            ),
            validatorVoteAccountLtHashes: Array(
                repeating: voteLtHash,
                count: sccpSolanaMaxValidators + 1
            ),
            validatorStakeAccountOpenings: input.validatorStakeAccountOpenings,
            validatorStakeAccountRawData: input.validatorStakeAccountRawData,
            validatorStakeAccountLtHashes: input.validatorStakeAccountLtHashes,
            stakeHistorySysvarOpening: input.stakeHistorySysvarOpening,
            stakeHistorySysvarRawData: input.stakeHistorySysvarRawData,
            stakeHistorySysvarAccountLtHash: input.stakeHistorySysvarAccountLtHash
        )
        XCTAssertThrowsError(try solanaSccpOpenedAccountsLtHashContributionsHash(oversizedVoteInput)) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("validatorVoteAccountOpenings"))
        }
    }

    func testSolanaAccountsLtHashChecksumUsesPureSwiftBlake3Vector() throws {
        XCTAssertEqual(
            try solanaSccpAccountsLtHashChecksum(Data(repeating: 0x99, count: 2_048)),
            "0x77a98713a20195e570cd12a8bdaaa355912663352b4b63c9f754c20f008860cc"
        )
        XCTAssertEqual(
            try solanaSccpAccountsLtHashChecksum(Data(repeating: 0, count: 2_048)),
            "0xbe2a8de3dcf46c94ce85cdc8e07ac308f4d8a95490d956c38d780fd610db0813"
        )
        XCTAssertEqual(
            try solanaSccpAccountsLtHashChecksum(Data((0..<2_048).map { UInt8($0 & 0xff) })),
            "0x1bdccfde0210a8ca178be19c6777cdb4b9a8fd24e7fe2b6b259b98e7aaaa0bb6"
        )
    }

    func testSolanaAccountLtHashDerivesFromOpeningsAndRawData() throws {
        func add(_ left: Data, _ right: Data) -> Data {
            var out = left
            for index in stride(from: 0, to: out.count, by: 2) {
                let current = UInt16(out[index]) | (UInt16(out[index + 1]) << 8)
                let addend = UInt16(right[index]) | (UInt16(right[index + 1]) << 8)
                let mixed = current &+ addend
                out[index] = UInt8(mixed & 0xff)
                out[index + 1] = UInt8(mixed >> 8)
            }
            return out
        }

        let voteOpening = SolanaSccpAccountOpeningInput(
            address: Data(repeating: 0x31, count: 32),
            owner: sccpSolanaVoteProgramId,
            lamports: 1_000_000,
            rentEpoch: 0,
            dataHash: "0x" + String(repeating: "91", count: 32)
        )
        let voteRawData = Data([1, 2, 3])
        let voteLtHash = try solanaSccpAccountLtHash(opening: voteOpening, rawData: voteRawData)
        XCTAssertEqual(voteLtHash.count, 2_048)
        XCTAssertEqual(
            try solanaSccpAccountsLtHashChecksum(voteLtHash),
            "0x56a868657e9113c76dc94321040b8f01a35ea4996c6fa235581510cd18be4bfe"
        )
        let maxRawData = Data((0..<65_536).map { UInt8($0 & 0xff) })
        let maxLtHash = try solanaSccpAccountLtHash(opening: voteOpening, rawData: maxRawData)
        XCTAssertEqual(
            try solanaSccpAccountsLtHashChecksum(maxLtHash),
            "0xc467c59f47747fdae4d87f8c79413ae24d3674ea3ca02aad0a1216a20d4fe147"
        )
        XCTAssertEqual(
            Data(maxLtHash.prefix(64)).hexEncodedString(),
            "c972db5d20a5a451a44daa674d0511382480d6e9060f750129723812e0e3c66a4" +
                "deddbb7975e2ff4d4c753aebcb703e61122d1ca1cfcd4f0c002a2cad30f4949"
        )
        XCTAssertEqual(
            Data(maxLtHash.suffix(64)).hexEncodedString(),
            "b4159fa2d334c4209bfb59997f7da42a56e2e921e0bbc4ebd916f3c55353b630" +
                "e26303b0af0b23e91870e9815f7ed6348395fbc7c0f07bf605da23589fa9fb51"
        )
        let zeroLamportsOpening = SolanaSccpAccountOpeningInput(
            address: Data(repeating: 0x33, count: 32),
            owner: sccpSolanaVoteProgramId,
            lamports: 0,
            rentEpoch: 0,
            dataHash: "0x" + String(repeating: "94", count: 32)
        )
        XCTAssertEqual(
            try solanaSccpAccountLtHash(opening: zeroLamportsOpening, rawData: voteRawData),
            Data(repeating: 0, count: 2_048)
        )
        XCTAssertEqual(
            try solanaSccpAccountsLtHashFromOpenings(
                openings: [voteOpening, zeroLamportsOpening],
                rawDataValues: [voteRawData, voteRawData]
            ),
            voteLtHash
        )

        let stakeOpening = SolanaSccpAccountOpeningInput(
            address: Data(repeating: 0x32, count: 32),
            owner: sccpSolanaStakeProgramId,
            lamports: 2_000_000,
            rentEpoch: 0,
            dataHash: "0x" + String(repeating: "92", count: 32)
        )
        let stakeHistoryOpening = SolanaSccpAccountOpeningInput(
            address: sccpSolanaStakeHistorySysvarId,
            owner: sccpSolanaSysvarProgramId,
            lamports: 1,
            rentEpoch: 0,
            dataHash: "0x" + String(repeating: "93", count: 32)
        )
        let stakeRawData = Data([4, 5, 6])
        let stakeHistoryRawData = Data([7, 8, 9])
        let stakeLtHash = try solanaSccpAccountLtHash(opening: stakeOpening, rawData: stakeRawData)
        let stakeHistoryLtHash = try solanaSccpAccountLtHash(
            opening: stakeHistoryOpening,
            rawData: stakeHistoryRawData
        )
        let unopenedLtHash = Data(repeating: 0x44, count: 2_048)
        let openedLtHash = try solanaSccpAccountsLtHashFromOpenings(
            openings: [voteOpening, stakeOpening, stakeHistoryOpening],
            rawDataValues: [voteRawData, stakeRawData, stakeHistoryRawData]
        )
        let accountsLtHash = add(openedLtHash, unopenedLtHash)
        let derivedInput = SolanaSccpOpenedAccountsLtHashContributionsInput(
            finalizedSlot: 1_296_096,
            accountInclusionRoot: "0x" + String(repeating: "77", count: 32),
            accountsLtHashChecksum: try solanaSccpAccountsLtHashChecksum(accountsLtHash),
            accountsLtHash: accountsLtHash,
            validatorVoteAccountOpenings: [voteOpening],
            validatorVoteAccountRawData: [voteRawData],
            validatorStakeAccountOpenings: [stakeOpening],
            validatorStakeAccountRawData: [stakeRawData],
            stakeHistorySysvarOpening: stakeHistoryOpening,
            stakeHistorySysvarRawData: stakeHistoryRawData
        )
        let precomputedInput = SolanaSccpOpenedAccountsLtHashContributionsInput(
            finalizedSlot: derivedInput.finalizedSlot,
            accountInclusionRoot: derivedInput.accountInclusionRoot,
            accountsLtHashChecksum: derivedInput.accountsLtHashChecksum,
            accountsLtHash: derivedInput.accountsLtHash,
            validatorVoteAccountOpenings: derivedInput.validatorVoteAccountOpenings,
            validatorVoteAccountRawData: derivedInput.validatorVoteAccountRawData,
            validatorVoteAccountLtHashes: [voteLtHash],
            validatorStakeAccountOpenings: derivedInput.validatorStakeAccountOpenings,
            validatorStakeAccountRawData: derivedInput.validatorStakeAccountRawData,
            validatorStakeAccountLtHashes: [stakeLtHash],
            stakeHistorySysvarOpening: derivedInput.stakeHistorySysvarOpening,
            stakeHistorySysvarRawData: derivedInput.stakeHistorySysvarRawData,
            stakeHistorySysvarAccountLtHash: stakeHistoryLtHash
        )
        XCTAssertEqual(try solanaSccpOpenedAccountsLtHashResidual(derivedInput), unopenedLtHash)
        XCTAssertEqual(
            try canonicalSolanaSccpOpenedAccountsLtHashContributionsBytes(derivedInput),
            try canonicalSolanaSccpOpenedAccountsLtHashContributionsBytes(precomputedInput)
        )
        XCTAssertEqual(
            try solanaSccpOpenedAccountsLtHashContributionsHash(derivedInput),
            try solanaSccpOpenedAccountsLtHashContributionsHash(precomputedInput)
        )
        var wrongVoteLtHash = voteLtHash
        wrongVoteLtHash[0] ^= 0x01
        let badPrecomputedInput = SolanaSccpOpenedAccountsLtHashContributionsInput(
            finalizedSlot: derivedInput.finalizedSlot,
            accountInclusionRoot: derivedInput.accountInclusionRoot,
            accountsLtHashChecksum: derivedInput.accountsLtHashChecksum,
            accountsLtHash: derivedInput.accountsLtHash,
            validatorVoteAccountOpenings: derivedInput.validatorVoteAccountOpenings,
            validatorVoteAccountRawData: derivedInput.validatorVoteAccountRawData,
            validatorVoteAccountLtHashes: [wrongVoteLtHash],
            validatorStakeAccountOpenings: derivedInput.validatorStakeAccountOpenings,
            validatorStakeAccountRawData: derivedInput.validatorStakeAccountRawData,
            validatorStakeAccountLtHashes: [stakeLtHash],
            stakeHistorySysvarOpening: derivedInput.stakeHistorySysvarOpening,
            stakeHistorySysvarRawData: derivedInput.stakeHistorySysvarRawData,
            stakeHistorySysvarAccountLtHash: stakeHistoryLtHash
        )
        XCTAssertThrowsError(
            try solanaSccpOpenedAccountsLtHashContributionsHash(badPrecomputedInput)
        ) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("validatorVoteAccountLtHashes[0]"))
        }
    }

    func testBuildsAccountsLtHashSourceStateProofRequest() async throws {
        func ltHash(_ value: UInt16) -> Data {
            var out = Data(repeating: 0, count: 2_048)
            for index in stride(from: 0, to: out.count, by: 2) {
                let word = value &+ UInt16(index / 2) &* 17
                out[index] = UInt8(word & 0xff)
                out[index + 1] = UInt8(word >> 8)
            }
            return out
        }
        func add(_ left: Data, _ right: Data) -> Data {
            var out = left
            for index in stride(from: 0, to: out.count, by: 2) {
                let mixed = (UInt16(out[index]) | (UInt16(out[index + 1]) << 8))
                    &+ (UInt16(right[index]) | (UInt16(right[index + 1]) << 8))
                out[index] = UInt8(mixed & 0xff)
                out[index + 1] = UInt8(mixed >> 8)
            }
            return out
        }
        let voteOpening = SolanaSccpAccountOpeningInput(
            address: Data(repeating: 0x31, count: 32),
            owner: sccpSolanaVoteProgramId,
            lamports: 1_000_000,
            rentEpoch: 0,
            dataHash: "0x" + String(repeating: "91", count: 32)
        )
        let stakeOpening = SolanaSccpAccountOpeningInput(
            address: Data(repeating: 0x32, count: 32),
            owner: sccpSolanaStakeProgramId,
            lamports: 2_000_000,
            rentEpoch: 0,
            dataHash: "0x" + String(repeating: "92", count: 32)
        )
        let stakeHistoryOpening = SolanaSccpAccountOpeningInput(
            address: sccpSolanaStakeHistorySysvarId,
            owner: sccpSolanaSysvarProgramId,
            lamports: 1,
            rentEpoch: 0,
            dataHash: "0x" + String(repeating: "93", count: 32)
        )
        let voteRawData = Data([1, 2, 3])
        let stakeRawData = Data([4, 5, 6])
        let stakeHistoryRawData = Data([7, 8, 9])
        let voteLtHash = try solanaSccpAccountLtHash(opening: voteOpening, rawData: voteRawData)
        let stakeLtHash = try solanaSccpAccountLtHash(opening: stakeOpening, rawData: stakeRawData)
        let stakeHistoryLtHash = try solanaSccpAccountLtHash(
            opening: stakeHistoryOpening,
            rawData: stakeHistoryRawData
        )
        let accountsLtHash = add(add(add(voteLtHash, stakeLtHash), stakeHistoryLtHash), ltHash(4))
        let opened = SolanaSccpOpenedAccountsLtHashContributionsInput(
            finalizedSlot: 1_296_096,
            accountInclusionRoot: "0x" + String(repeating: "77", count: 32),
            accountsLtHashChecksum: try solanaSccpAccountsLtHashChecksum(accountsLtHash),
            accountsLtHash: accountsLtHash,
            validatorVoteAccountOpenings: [voteOpening],
            validatorVoteAccountRawData: [voteRawData],
            validatorVoteAccountLtHashes: [voteLtHash],
            validatorStakeAccountOpenings: [stakeOpening],
            validatorStakeAccountRawData: [stakeRawData],
            validatorStakeAccountLtHashes: [stakeLtHash],
            stakeHistorySysvarOpening: stakeHistoryOpening,
            stakeHistorySysvarRawData: stakeHistoryRawData,
            stakeHistorySysvarAccountLtHash: stakeHistoryLtHash
        )
        let parentBankHash = String(repeating: "c0", count: 32)
        let blockhash = String(repeating: "42", count: 32)
        let bankHash = try solanaSccpAgaveBankHash(
            parentBankHash: parentBankHash,
            bankSignatureCount: 8,
            blockhash: blockhash,
            accountsLtHash: accountsLtHash
        )
        let zeroAccountsLtHash = Data(repeating: 0, count: 2_048)
        let zeroAccountsLtHashChecksum = try solanaSccpAccountsLtHashChecksum(zeroAccountsLtHash)
        XCTAssertTrue(zeroAccountsLtHashChecksum.hasPrefix("0x"))
        XCTAssertThrowsError(try solanaSccpAgaveBankHash(
            parentBankHash: parentBankHash,
            bankSignatureCount: 8,
            blockhash: blockhash,
            accountsLtHash: zeroAccountsLtHash
        ))
        XCTAssertThrowsError(try solanaSccpOpenedAccountsLtHashContributionsHash(
            SolanaSccpOpenedAccountsLtHashContributionsInput(
                finalizedSlot: opened.finalizedSlot,
                accountInclusionRoot: opened.accountInclusionRoot,
                accountsLtHashChecksum: zeroAccountsLtHashChecksum,
                accountsLtHash: zeroAccountsLtHash,
                validatorVoteAccountOpenings: opened.validatorVoteAccountOpenings,
                validatorVoteAccountRawData: opened.validatorVoteAccountRawData,
                validatorVoteAccountLtHashes: opened.validatorVoteAccountLtHashes,
                validatorStakeAccountOpenings: opened.validatorStakeAccountOpenings,
                validatorStakeAccountRawData: opened.validatorStakeAccountRawData,
                validatorStakeAccountLtHashes: opened.validatorStakeAccountLtHashes,
                stakeHistorySysvarOpening: opened.stakeHistorySysvarOpening,
                stakeHistorySysvarRawData: opened.stakeHistorySysvarRawData,
                stakeHistorySysvarAccountLtHash: opened.stakeHistorySysvarAccountLtHash
            )
        ))
        let witness = SolanaSccpWitnessInput(
            finalizedSlot: opened.finalizedSlot,
            parentSlot: opened.finalizedSlot - 1,
            bankSignatureCount: 8,
            parentBankHash: parentBankHash,
            blockhash: blockhash,
            bankHash: bankHash,
            transactionStatusRoot: String(repeating: "bb", count: 32),
            messageProofHash: String(repeating: "cc", count: 32),
            accountInclusionRoot: opened.accountInclusionRoot,
            accountsLtHashChecksum: opened.accountsLtHashChecksum,
            accountsLtHash: accountsLtHash,
            transactionSignature: Self.solanaSignature55,
            emitterProgramId: Self.solanaProgram42,
            messageId: String(repeating: "dd", count: 32),
            payloadHash: String(repeating: "ee", count: 32),
            commitmentRoot: String(repeating: "12", count: 32),
            sourceEventDigest: String(repeating: "34", count: 32),
            sourceStateVerifierHash: String(repeating: "aa", count: 32),
            statementHash: String(repeating: "56", count: 32),
            destinationBindingHash: String(repeating: "78", count: 32)
        )

        let request = try buildSolanaSccpAccountsLtHashProofRequest(
            witness: witness,
            openedAccounts: opened
        )
        var mismatchedWitnessLtHash = accountsLtHash
        mismatchedWitnessLtHash[0] = mismatchedWitnessLtHash[0] ^ 0x01
        XCTAssertThrowsError(
            try buildSolanaSccpAccountsLtHashProofRequest(
                witness: SolanaSccpWitnessInput(
                    finalizedSlot: witness.finalizedSlot,
                    parentSlot: witness.parentSlot,
                    bankSignatureCount: witness.bankSignatureCount,
                    parentBankHash: witness.parentBankHash,
                    blockhash: witness.blockhash,
                    bankHash: witness.bankHash,
                    transactionStatusRoot: witness.transactionStatusRoot,
                    messageProofHash: witness.messageProofHash,
                    accountInclusionRoot: witness.accountInclusionRoot,
                    accountsLtHashChecksum: witness.accountsLtHashChecksum,
                    accountsLtHash: mismatchedWitnessLtHash,
                    transactionSignature: witness.transactionSignature,
                    emitterProgramId: witness.emitterProgramId,
                    messageId: witness.messageId,
                    payloadHash: witness.payloadHash,
                    commitmentRoot: witness.commitmentRoot,
                    sourceEventDigest: witness.sourceEventDigest,
                    sourceStateVerifierHash: witness.sourceStateVerifierHash,
                    statementHash: witness.statementHash,
                    destinationBindingHash: witness.destinationBindingHash
                ),
                openedAccounts: opened
            )
        ) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("accountsLtHash"))
        }

        XCTAssertEqual(request.version, 1)
        XCTAssertEqual(request.proofFamily, "stark-fri-v1")
        XCTAssertEqual(request.circuitId, sccpSolanaAccountsLtHashOpenVerifyCircuitIdV1)
        XCTAssertEqual(request.parameterSet, "fastpq-lane-balanced")
        XCTAssertEqual(request.sourceStateVerifierId, sccpSolanaMainnetAccountsDbVerifierIdV1)
        XCTAssertEqual(request.sourceStateVerifierHash, "0x" + String(repeating: "aa", count: 32))
        XCTAssertEqual(
            request.accountsLtHashProofPublicInputsHash,
            try solanaSccpAccountsLtHashProofPublicInputsHash(
                sourceDomain: sccpDomainSolana,
                finalizedSlot: witness.finalizedSlot,
                parentSlot: witness.parentSlot,
                bankSignatureCount: witness.bankSignatureCount,
                parentBankHash: witness.parentBankHash,
                bankHash: witness.bankHash,
                blockhash: witness.blockhash,
                transactionStatusRoot: witness.transactionStatusRoot,
                accountInclusionRoot: witness.accountInclusionRoot,
                accountsLtHashChecksum: witness.accountsLtHashChecksum,
                accountsLtHash: accountsLtHash
            )
        )
        XCTAssertThrowsError(
            try canonicalSolanaSccpAccountsLtHashProofPublicInputsBytes(
                sourceDomain: sccpDomainSolana,
                finalizedSlot: witness.finalizedSlot,
                parentSlot: witness.parentSlot,
                bankSignatureCount: witness.bankSignatureCount,
                parentBankHash: witness.parentBankHash,
                bankHash: "0x" + String(repeating: "44", count: 32),
                blockhash: witness.blockhash,
                transactionStatusRoot: witness.transactionStatusRoot,
                accountInclusionRoot: witness.accountInclusionRoot,
                accountsLtHashChecksum: witness.accountsLtHashChecksum,
                accountsLtHash: accountsLtHash
            )
        ) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("bankHash"))
        }
        XCTAssertThrowsError(
            try solanaSccpAccountsLtHashProofPublicInputsHash(
                sourceDomain: sccpDomainSolana,
                finalizedSlot: witness.finalizedSlot,
                parentSlot: witness.parentSlot,
                bankSignatureCount: witness.bankSignatureCount,
                parentBankHash: witness.parentBankHash,
                bankHash: witness.bankHash,
                blockhash: witness.blockhash,
                transactionStatusRoot: witness.transactionStatusRoot,
                accountInclusionRoot: witness.accountInclusionRoot,
                accountsLtHashChecksum: "0x" + String(repeating: "44", count: 32),
                accountsLtHash: accountsLtHash
            )
        ) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("accountsLtHashChecksum"))
        }
        XCTAssertEqual(
            request.openedAccountsLtHashContributionsHash,
            try solanaSccpOpenedAccountsLtHashContributionsHash(opened)
        )
        XCTAssertEqual(
            request.openedAccountsLtHashResidualChecksum,
            try solanaSccpOpenedAccountsLtHashResidualChecksum(opened)
        )
        XCTAssertEqual(
            request.accountCommitmentBytes,
            try canonicalSolanaSccpAccountsLtHashCommitmentBytes(witness: witness, openedAccounts: opened)
        )
        XCTAssertEqual(
            request.verificationContextBytes,
            try canonicalSolanaSccpAccountsLtHashVerificationContextBytes(witness: witness, openedAccounts: opened)
        )
        XCTAssertEqual(
            request.publicInputColumns,
            try solanaSccpAccountsLtHashPublicInputColumns(witness: witness, openedAccounts: opened)
        )
        XCTAssertEqual(request.publicInputColumns[1][0], Self.solanaMainnetGenesisPublicInput)
        XCTAssertEqual(request.publicInputColumns[12][0], request.openedAccountsLtHashContributionsHash)
        XCTAssertEqual(request.publicInputColumns[13][0], request.openedAccountsLtHashResidualChecksum)
        XCTAssertNotNil(
            request.schemaDescriptor.range(of: Data("opened_accounts_lt_hash_residual_checksum".utf8))
        )
        XCTAssertNotNil(request.schemaDescriptor.range(of: Data("mainnet_genesis_hash".utf8)))
        XCTAssertNotNil(request.schemaDescriptor.range(of: Data("source_state_verifier_id".utf8)))
        XCTAssertNotNil(request.schemaDescriptor.range(of: Data(sccpSolanaMainnetAccountsDbVerifierIdV1.utf8)))
        XCTAssertNotNil(request.schemaDescriptor.range(of: Data("source_state_verifier_hash".utf8)))
        XCTAssertNotNil(
            request.schemaDescriptor.range(
                of: Data(hexString: String(request.sourceStateVerifierHash.dropFirst(2)))!
            )
        )
        XCTAssertEqual(
            request.fastpqTransitions.map(\.key),
            [
                "sccp:solana:accounts-lt:v1:statement",
                "sccp:solana:accounts-lt:v1:accounts",
                "sccp:solana:accounts-lt:v1:opened-contributions",
                "sccp:solana:accounts-lt:v1:residual",
                "sccp:solana:accounts-lt:v1:context",
            ]
        )
        XCTAssertEqual(request.fastpqPublicInputs.oldRoot, "0x" + parentBankHash)
        XCTAssertEqual(request.fastpqPublicInputs.newRoot, bankHash)
        let proofCapsule = try wrapSolanaSccpSourceStateVerificationProof(
            proofBytes: Data([1, 2, 3]),
            request: request
        )
        XCTAssertEqual(proofCapsule.version, request.version)
        XCTAssertEqual(proofCapsule.proofFamily, request.proofFamily)
        XCTAssertEqual(proofCapsule.circuitId, request.circuitId)
        XCTAssertEqual(proofCapsule.proofBytes, Data([1, 2, 3]))
        XCTAssertEqual(proofCapsule.proofBase64, "AQID")
        var exposedProofBytes = proofCapsule.proofBytes
        exposedProofBytes[0] = 9
        XCTAssertEqual(proofCapsule.proofBytes, Data([1, 2, 3]))
        XCTAssertEqual(proofCapsule.proofBase64, "AQID")
        XCTAssertEqual(
            try solanaSccpAccountsLtHashProofHash(proofCapsule),
            try solanaSccpAccountsLtHashProofHash(
                SolanaSccpSourceStateVerificationProof(
                    circuitId: request.circuitId,
                    proofBytes: Data([1, 2, 3])
                )
            )
        )
        XCTAssertThrowsError(
            try wrapSolanaSccpSourceStateVerificationProof(
                proofBytes: Data(repeating: 0, count: 2),
                request: request
            )
        ) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .allZeroProof)
        }
        let oversizedProofBytes = Data(repeating: 1, count: sccpSourceStateMaxProofBytes + 1)
        XCTAssertThrowsError(
            try wrapSolanaSccpSourceStateVerificationProof(
                proofBytes: oversizedProofBytes,
                request: request
            )
        ) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("proofBytes"))
        }
        XCTAssertThrowsError(
            try canonicalSolanaSccpSourceStateVerificationProofBytes(
                SolanaSccpSourceStateVerificationProof(
                    circuitId: request.circuitId,
                    proofBytes: oversizedProofBytes
                )
            )
        ) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("proofBytes"))
        }
        XCTAssertThrowsError(
            try canonicalSolanaSccpSourceStateVerificationProofBytes(
                SolanaSccpSourceStateVerificationProof(
                    proofFamily: String(repeating: "x", count: sccpSourceStateMaxProofLabelBytes + 1),
                    circuitId: request.circuitId,
                    proofBytes: Data([1])
                )
            )
        ) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("proofFamily"))
        }
        XCTAssertThrowsError(
            try canonicalSolanaSccpSourceStateVerificationProofBytes(
                SolanaSccpSourceStateVerificationProof(
                    circuitId: String(repeating: "x", count: sccpSourceStateMaxProofLabelBytes + 1),
                    proofBytes: Data([1])
                )
            )
        ) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("circuitId"))
        }
        var wrongGenesisColumns = request.publicInputColumns
        wrongGenesisColumns[1][0] = "0x" + String(repeating: "aa", count: 32)
        let wrongGenesisRequest = SolanaSccpAccountsLtHashProofRequest(
            version: request.version,
            proofFamily: request.proofFamily,
            circuitId: request.circuitId,
            parameterSet: request.parameterSet,
            sourceDomain: request.sourceDomain,
            finalizedSlot: request.finalizedSlot,
            parentSlot: request.parentSlot,
            sourceStateVerifierId: request.sourceStateVerifierId,
            sourceStateVerifierHash: request.sourceStateVerifierHash,
            accountsLtHashProofPublicInputsHash: request.accountsLtHashProofPublicInputsHash,
            openedAccountsLtHashContributionsHash: request.openedAccountsLtHashContributionsHash,
            openedAccountsLtHashResidualChecksum: request.openedAccountsLtHashResidualChecksum,
            statementBytes: request.statementBytes,
            accountCommitmentBytes: request.accountCommitmentBytes,
            verificationContextBytes: request.verificationContextBytes,
            schemaDescriptor: request.schemaDescriptor,
            publicInputColumns: wrongGenesisColumns,
            fastpqPublicInputs: request.fastpqPublicInputs,
            fastpqTransitions: request.fastpqTransitions
        )
        XCTAssertThrowsError(
            try wrapSolanaSccpSourceStateVerificationProof(
                proofBytes: Data([1]),
                request: wrongGenesisRequest
            )
        ) { error in
            XCTAssertEqual(
                error as? SolanaSccpProverError,
                .invalidString("request.publicInputColumns.mainnet_genesis_hash")
            )
        }
        var wrongResidualColumns = request.publicInputColumns
        wrongResidualColumns[13][0] = "0x" + String(repeating: "cc", count: 32)
        let wrongResidualRequest = SolanaSccpAccountsLtHashProofRequest(
            version: request.version,
            proofFamily: request.proofFamily,
            circuitId: request.circuitId,
            parameterSet: request.parameterSet,
            sourceDomain: request.sourceDomain,
            finalizedSlot: request.finalizedSlot,
            parentSlot: request.parentSlot,
            sourceStateVerifierId: request.sourceStateVerifierId,
            sourceStateVerifierHash: request.sourceStateVerifierHash,
            accountsLtHashProofPublicInputsHash: request.accountsLtHashProofPublicInputsHash,
            openedAccountsLtHashContributionsHash: request.openedAccountsLtHashContributionsHash,
            openedAccountsLtHashResidualChecksum: request.openedAccountsLtHashResidualChecksum,
            statementBytes: request.statementBytes,
            accountCommitmentBytes: request.accountCommitmentBytes,
            verificationContextBytes: request.verificationContextBytes,
            schemaDescriptor: request.schemaDescriptor,
            publicInputColumns: wrongResidualColumns,
            fastpqPublicInputs: request.fastpqPublicInputs,
            fastpqTransitions: request.fastpqTransitions
        )
        XCTAssertThrowsError(
            try wrapSolanaSccpSourceStateVerificationProof(
                proofBytes: Data([1]),
                request: wrongResidualRequest
            )
        ) { error in
            XCTAssertEqual(
                error as? SolanaSccpProverError,
                .invalidString("request.publicInputColumns.opened_accounts_lt_hash_residual_checksum")
            )
        }
        let staleAccountsHashRequest = Self.accountsLtHashRequest(
            request,
            accountsLtHashProofPublicInputsHash: "0x" + String(repeating: "cc", count: 32)
        )
        XCTAssertThrowsError(
            try wrapSolanaSccpSourceStateVerificationProof(
                proofBytes: Data([1]),
                request: staleAccountsHashRequest
            )
        ) { error in
            XCTAssertEqual(
                error as? SolanaSccpProverError,
                .invalidString("request.accountsLtHashProofPublicInputsHash")
            )
        }
        let wrongAccountsDsidInputs = SolanaSccpAccountsLtHashFastpqPublicInputs(
            dsid: "0x" + String(repeating: "00", count: 16),
            slot: request.fastpqPublicInputs.slot,
            oldRoot: request.fastpqPublicInputs.oldRoot,
            newRoot: request.fastpqPublicInputs.newRoot,
            permRoot: request.fastpqPublicInputs.permRoot,
            txSetHash: request.fastpqPublicInputs.txSetHash
        )
        let wrongAccountsDsidRequest = Self.accountsLtHashRequest(
            request,
            fastpqPublicInputs: wrongAccountsDsidInputs
        )
        XCTAssertThrowsError(
            try wrapSolanaSccpSourceStateVerificationProof(
                proofBytes: Data([1]),
                request: wrongAccountsDsidRequest
            )
        ) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("request.fastpqPublicInputs.dsid"))
        }
        let wrongAccountsTxInputs = SolanaSccpAccountsLtHashFastpqPublicInputs(
            dsid: request.fastpqPublicInputs.dsid,
            slot: request.fastpqPublicInputs.slot,
            oldRoot: request.fastpqPublicInputs.oldRoot,
            newRoot: request.fastpqPublicInputs.newRoot,
            permRoot: request.fastpqPublicInputs.permRoot,
            txSetHash: "0x" + String(repeating: "cc", count: 32)
        )
        let wrongAccountsTxRequest = Self.accountsLtHashRequest(
            request,
            fastpqPublicInputs: wrongAccountsTxInputs
        )
        XCTAssertThrowsError(
            try wrapSolanaSccpSourceStateVerificationProof(
                proofBytes: Data([1]),
                request: wrongAccountsTxRequest
            )
        ) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("request.fastpqPublicInputs.txSetHash"))
        }
        var wrongTransitions = request.fastpqTransitions
        wrongTransitions[0] = SolanaSccpAccountsLtHashFastpqTransition(
            key: wrongTransitions[0].key,
            operation: wrongTransitions[0].operation,
            oldValue: wrongTransitions[0].oldValue,
            newValue: Data([0])
        )
        let wrongTransitionRequest = SolanaSccpAccountsLtHashProofRequest(
            version: request.version,
            proofFamily: request.proofFamily,
            circuitId: request.circuitId,
            parameterSet: request.parameterSet,
            sourceDomain: request.sourceDomain,
            finalizedSlot: request.finalizedSlot,
            parentSlot: request.parentSlot,
            sourceStateVerifierId: request.sourceStateVerifierId,
            sourceStateVerifierHash: request.sourceStateVerifierHash,
            accountsLtHashProofPublicInputsHash: request.accountsLtHashProofPublicInputsHash,
            openedAccountsLtHashContributionsHash: request.openedAccountsLtHashContributionsHash,
            openedAccountsLtHashResidualChecksum: request.openedAccountsLtHashResidualChecksum,
            statementBytes: request.statementBytes,
            accountCommitmentBytes: request.accountCommitmentBytes,
            verificationContextBytes: request.verificationContextBytes,
            schemaDescriptor: request.schemaDescriptor,
            publicInputColumns: request.publicInputColumns,
            fastpqPublicInputs: request.fastpqPublicInputs,
            fastpqTransitions: wrongTransitions
        )
        XCTAssertThrowsError(
            try wrapSolanaSccpSourceStateVerificationProof(
                proofBytes: Data([1]),
                request: wrongTransitionRequest
            )
        ) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("request.fastpqTransitions"))
        }
        var wrongOldValueTransitions = request.fastpqTransitions
        wrongOldValueTransitions[0] = SolanaSccpAccountsLtHashFastpqTransition(
            key: wrongOldValueTransitions[0].key,
            operation: wrongOldValueTransitions[0].operation,
            oldValue: Data([0]),
            newValue: wrongOldValueTransitions[0].newValue
        )
        let wrongOldValueRequest = SolanaSccpAccountsLtHashProofRequest(
            version: request.version,
            proofFamily: request.proofFamily,
            circuitId: request.circuitId,
            parameterSet: request.parameterSet,
            sourceDomain: request.sourceDomain,
            finalizedSlot: request.finalizedSlot,
            parentSlot: request.parentSlot,
            sourceStateVerifierId: request.sourceStateVerifierId,
            sourceStateVerifierHash: request.sourceStateVerifierHash,
            accountsLtHashProofPublicInputsHash: request.accountsLtHashProofPublicInputsHash,
            openedAccountsLtHashContributionsHash: request.openedAccountsLtHashContributionsHash,
            openedAccountsLtHashResidualChecksum: request.openedAccountsLtHashResidualChecksum,
            statementBytes: request.statementBytes,
            accountCommitmentBytes: request.accountCommitmentBytes,
            verificationContextBytes: request.verificationContextBytes,
            schemaDescriptor: request.schemaDescriptor,
            publicInputColumns: request.publicInputColumns,
            fastpqPublicInputs: request.fastpqPublicInputs,
            fastpqTransitions: wrongOldValueTransitions
        )
        XCTAssertThrowsError(
            try wrapSolanaSccpSourceStateVerificationProof(
                proofBytes: Data([1]),
                request: wrongOldValueRequest
            )
        ) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("request.fastpqTransitions"))
        }
        var seenAccountsRequest: SolanaSccpAccountsLtHashProofRequest?
        let sourceStateProver = SolanaSccpSourceStateProver(
            accountsLtHashProveFunction: { linkedRequest in
                seenAccountsRequest = linkedRequest
                XCTAssertEqual(
                    linkedRequest.circuitId,
                    sccpSolanaAccountsLtHashOpenVerifyCircuitIdV1
                )
                return Data([1, 2, 3])
            }
        )
        let linkedProof = try await sourceStateProver.proveAccountsLtHash(
            witness: witness,
            openedAccounts: opened
        )
        XCTAssertEqual(seenAccountsRequest?.circuitId, request.circuitId)
        XCTAssertEqual(linkedProof.circuitId, request.circuitId)
        XCTAssertEqual(linkedProof.proofBytes, Data([1, 2, 3]))
        XCTAssertEqual(linkedProof.proofBase64, "AQID")
        do {
            _ = try await SolanaSccpSourceStateProver().proveAccountsLtHash(request: request)
            XCTFail("expected missing source-state prover")
        } catch {
            XCTAssertEqual(error as? SolanaSccpProverError, .localProverUnavailable)
        }
        let missingStatementRequest = SolanaSccpAccountsLtHashProofRequest(
            version: request.version,
            proofFamily: request.proofFamily,
            circuitId: request.circuitId,
            parameterSet: request.parameterSet,
            sourceDomain: request.sourceDomain,
            finalizedSlot: request.finalizedSlot,
            parentSlot: request.parentSlot,
            sourceStateVerifierId: request.sourceStateVerifierId,
            sourceStateVerifierHash: request.sourceStateVerifierHash,
            accountsLtHashProofPublicInputsHash: request.accountsLtHashProofPublicInputsHash,
            openedAccountsLtHashContributionsHash: request.openedAccountsLtHashContributionsHash,
            openedAccountsLtHashResidualChecksum: request.openedAccountsLtHashResidualChecksum,
            statementBytes: Data(),
            accountCommitmentBytes: request.accountCommitmentBytes,
            verificationContextBytes: request.verificationContextBytes,
            schemaDescriptor: request.schemaDescriptor,
            publicInputColumns: request.publicInputColumns,
            fastpqPublicInputs: request.fastpqPublicInputs,
            fastpqTransitions: request.fastpqTransitions
        )
        XCTAssertThrowsError(
            try wrapSolanaSccpSourceStateVerificationProof(
                proofBytes: Data([1]),
                request: missingStatementRequest
            )
        ) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("request.statementBytes"))
        }
        var rejectedAccountsCallbackRan = false
        let guardingAccountsProver = SolanaSccpSourceStateProver(
            accountsLtHashProveFunction: { _ in
                rejectedAccountsCallbackRan = true
                return Data([1])
            }
        )
        do {
            _ = try await guardingAccountsProver.proveAccountsLtHash(request: missingStatementRequest)
            XCTFail("expected malformed AccountsLtHash request to fail before callback")
        } catch {
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("request.statementBytes"))
            XCTAssertFalse(rejectedAccountsCallbackRan)
        }
        let expectedStatementBytes = request.statementBytes
        var exposedStatement = request.statementBytes
        exposedStatement[exposedStatement.startIndex] = exposedStatement[exposedStatement.startIndex] == 0 ? 1 : 0
        XCTAssertEqual(request.statementBytes, expectedStatementBytes)
        let expectedTransitionValue = request.fastpqTransitions[0].newValue
        var exposedTransitionValue = request.fastpqTransitions[0].newValue
        exposedTransitionValue[exposedTransitionValue.startIndex] =
            exposedTransitionValue[exposedTransitionValue.startIndex] == 0 ? 1 : 0
        XCTAssertEqual(request.fastpqTransitions[0].newValue, expectedTransitionValue)
        XCTAssertThrowsError(try buildSolanaSccpAccountsLtHashProofRequest(
            witness: SolanaSccpWitnessInput(
                finalizedSlot: witness.finalizedSlot,
                parentSlot: witness.parentSlot,
                bankSignatureCount: witness.bankSignatureCount,
                parentBankHash: witness.parentBankHash,
                blockhash: witness.blockhash,
                bankHash: witness.bankHash,
                transactionStatusRoot: witness.transactionStatusRoot,
                messageProofHash: witness.messageProofHash,
                accountInclusionRoot: witness.accountInclusionRoot,
                accountsLtHashChecksum: witness.accountsLtHashChecksum,
                accountsLtHash: witness.accountsLtHash,
                transactionSignature: witness.transactionSignature,
                emitterProgramId: witness.emitterProgramId,
                messageId: witness.messageId,
                payloadHash: witness.payloadHash,
                commitmentRoot: witness.commitmentRoot,
                sourceEventDigest: witness.sourceEventDigest,
                sourceStateVerifierHash: sccpZeroHashV1,
                statementHash: witness.statementHash,
                destinationBindingHash: witness.destinationBindingHash
            ),
            openedAccounts: opened
        ))
        XCTAssertThrowsError(try buildSolanaSccpAccountsLtHashProofRequest(
            witness: SolanaSccpWitnessInput(
                finalizedSlot: witness.finalizedSlot,
                parentSlot: witness.parentSlot,
                bankSignatureCount: witness.bankSignatureCount,
                parentBankHash: witness.parentBankHash,
                blockhash: witness.blockhash,
                bankHash: witness.bankHash,
                transactionStatusRoot: witness.transactionStatusRoot,
                messageProofHash: witness.messageProofHash,
                accountInclusionRoot: witness.accountInclusionRoot,
                accountsLtHashChecksum: witness.accountsLtHashChecksum,
                accountsLtHash: witness.accountsLtHash,
                transactionSignature: witness.transactionSignature,
                emitterProgramId: witness.emitterProgramId,
                messageId: witness.messageId,
                payloadHash: witness.payloadHash,
                commitmentRoot: witness.commitmentRoot,
                sourceEventDigest: witness.sourceEventDigest,
                sourceStateVerifierHash: sccpSolanaTemplateSourceStateVerifierHashV1,
                statementHash: witness.statementHash,
                destinationBindingHash: witness.destinationBindingHash
            ),
            openedAccounts: opened
        ))
    }

    func testBuildsSolanaFullLightClientAuditRoleProofRequests() async throws {
        let voteOpening = SolanaSccpAccountOpeningInput(
            address: Data(repeating: 0x31, count: 32),
            owner: sccpSolanaVoteProgramId,
            lamports: 1_000_000,
            rentEpoch: 0,
            dataHash: "0x" + String(repeating: "91", count: 32)
        )
        let stakeOpening = SolanaSccpAccountOpeningInput(
            address: Data(repeating: 0x32, count: 32),
            owner: sccpSolanaStakeProgramId,
            lamports: 2_000_000,
            rentEpoch: 0,
            dataHash: "0x" + String(repeating: "92", count: 32)
        )
        let stakeHistoryOpening = SolanaSccpAccountOpeningInput(
            address: sccpSolanaStakeHistorySysvarId,
            owner: sccpSolanaSysvarProgramId,
            lamports: 1,
            rentEpoch: 0,
            dataHash: "0x" + String(repeating: "93", count: 32)
        )
        let unopenedOpening = SolanaSccpAccountOpeningInput(
            address: Data(repeating: 0x34, count: 32),
            owner: sccpSolanaStakeProgramId,
            lamports: 3_000_000,
            rentEpoch: 0,
            dataHash: "0x" + String(repeating: "94", count: 32)
        )
        let voteRawData = Data([1, 2, 3])
        let stakeRawData = Data([4, 5, 6])
        let stakeHistoryRawData = Data([7, 8, 9])
        let unopenedRawData = Data([10, 11, 12])
        let accountsLtHash = try solanaSccpAccountsLtHashFromOpenings(
            openings: [voteOpening, stakeOpening, stakeHistoryOpening, unopenedOpening],
            rawDataValues: [voteRawData, stakeRawData, stakeHistoryRawData, unopenedRawData]
        )
        let parentBankHash = String(repeating: "c0", count: 32)
        let blockhash = String(repeating: "42", count: 32)
        let bankHash = try solanaSccpAgaveBankHash(
            parentBankHash: parentBankHash,
            bankSignatureCount: 8,
            blockhash: blockhash,
            accountsLtHash: accountsLtHash
        )
        let opened = SolanaSccpOpenedAccountsLtHashContributionsInput(
            finalizedSlot: 1_296_096,
            accountInclusionRoot: "0x" + String(repeating: "77", count: 32),
            accountsLtHashChecksum: try solanaSccpAccountsLtHashChecksum(accountsLtHash),
            accountsLtHash: accountsLtHash,
            validatorVoteAccountOpenings: [voteOpening],
            validatorVoteAccountRawData: [voteRawData],
            validatorStakeAccountOpenings: [stakeOpening],
            validatorStakeAccountRawData: [stakeRawData],
            stakeHistorySysvarOpening: stakeHistoryOpening,
            stakeHistorySysvarRawData: stakeHistoryRawData
        )
        let sourceStateVerifierHash = String(repeating: "99", count: 32)
        let sourceTrustAnchorHash = String(repeating: "44", count: 32)
        let consensusVerifierHash = String(repeating: "55", count: 32)
        let messageInclusionVerifierHash = String(repeating: "66", count: 32)
        let finalityPolicyHash = String(repeating: "88", count: 32)
        let deploymentReceiptHash = String(repeating: "aa", count: 32)
        let towerVerifierHash = String(repeating: "b1", count: 32)
        let accountsdbVerifierHash = String(repeating: "c2", count: 32)
        let bankVerifierHash = String(repeating: "d3", count: 32)
        let sourceVerifierMaterialHash = try sccpSourceVerifierMaterialHash(
            sourceDomain: sccpDomainSolana,
            sourceTrustAnchorHash: sourceTrustAnchorHash,
            consensusVerifierHash: consensusVerifierHash,
            messageInclusionVerifierHash: messageInclusionVerifierHash,
            finalityPolicyHash: finalityPolicyHash,
            sourceStateVerifierHash: sourceStateVerifierHash
        )
        let sourceAdapterDeploymentHash = try sccpSourceAdapterEngineDeploymentHash(
            sourceDomain: sccpDomainSolana,
            sourceTrustAnchorHash: sourceTrustAnchorHash,
            consensusVerifierHash: consensusVerifierHash,
            messageInclusionVerifierHash: messageInclusionVerifierHash,
            finalityPolicyHash: finalityPolicyHash,
            deploymentReceiptHash: deploymentReceiptHash,
            sourceStateVerifierHash: sourceStateVerifierHash,
            solanaTowerReplayVerifierHash: towerVerifierHash,
            solanaFullAccountsdbLatticeVerifierHash: accountsdbVerifierHash,
            solanaBankForkChoiceVerifierHash: bankVerifierHash
        )
        let fullLightClientGateHash = try sccpSolanaFullLightClientGateHash(
            sourceTrustAnchorHash: sourceTrustAnchorHash,
            consensusVerifierHash: consensusVerifierHash,
            messageInclusionVerifierHash: messageInclusionVerifierHash,
            finalityPolicyHash: finalityPolicyHash,
            deploymentReceiptHash: deploymentReceiptHash,
            solanaTowerReplayVerifierHash: towerVerifierHash,
            solanaFullAccountsdbLatticeVerifierHash: accountsdbVerifierHash,
            solanaBankForkChoiceVerifierHash: bankVerifierHash,
            sourceStateVerifierHash: sourceStateVerifierHash
        )
        func witnessInput(
            sourceAdapterDeploymentHash overrideSourceAdapterDeploymentHash: String = sourceAdapterDeploymentHash,
            sourceAdapterDeploymentReceiptHash overrideSourceAdapterDeploymentReceiptHash: String = deploymentReceiptHash,
            accountsLtHash overrideAccountsLtHash: Data? = accountsLtHash
        ) -> SolanaSccpWitnessInput {
            SolanaSccpWitnessInput(
                finalizedSlot: opened.finalizedSlot,
                parentSlot: opened.finalizedSlot - 1,
                bankSignatureCount: 8,
                parentBankHash: parentBankHash,
                blockhash: blockhash,
                bankHash: bankHash,
                transactionStatusRoot: String(repeating: "bb", count: 32),
                messageProofHash: String(repeating: "cc", count: 32),
                accountInclusionRoot: opened.accountInclusionRoot,
                accountsLtHashChecksum: opened.accountsLtHashChecksum,
                accountsLtHash: overrideAccountsLtHash,
                transactionSignature: Self.solanaSignature55,
                emitterProgramId: Self.solanaProgram42,
                messageId: String(repeating: "dd", count: 32),
                payloadHash: String(repeating: "ee", count: 32),
                commitmentRoot: String(repeating: "12", count: 32),
                sourceEventDigest: String(repeating: "34", count: 32),
                sourceStateVerifierHash: sourceStateVerifierHash,
                statementHash: String(repeating: "56", count: 32),
                destinationBindingHash: String(repeating: "78", count: 32),
                sourceAdapterDeploymentHash: overrideSourceAdapterDeploymentHash,
                sourceAdapterDeploymentReceiptHash: overrideSourceAdapterDeploymentReceiptHash
            )
        }
        let witness = witnessInput()
        func auditInput(
            fullLightClientGateHash overrideFullLightClientGateHash: String? = nil,
            sourceVerifierMaterialHash overrideSourceVerifierMaterialHash: String? = nil,
            sourceAdapterDeploymentHash overrideSourceAdapterDeploymentHash: String? = nil,
            sourceAdapterDeploymentReceiptHash overrideSourceAdapterDeploymentReceiptHash: String? = nil,
            towerVerifierHash overrideTowerVerifierHash: String? = nil,
            accountsdbVerifierHash overrideAccountsdbVerifierHash: String? = nil,
            bankVerifierHash overrideBankVerifierHash: String? = nil,
            witness overrideWitness: SolanaSccpWitnessInput? = nil
        ) -> SolanaSccpFullLightClientAuditProofInput {
            SolanaSccpFullLightClientAuditProofInput(
                witness: overrideWitness ?? witness,
                openedAccounts: opened,
                accountsLtHashProof: SolanaSccpSourceStateVerificationProof(
                    circuitId: sccpSolanaAccountsLtHashOpenVerifyCircuitIdV1,
                    proofBytes: Data([1, 2, 3, 4])
                ),
                rootedSlot: 1_296_065,
                towerVoteSlots: Array(1_296_066...1_296_096),
                epochStakeRoot: String(repeating: "13", count: 32),
                stakeActivationHash: String(repeating: "14", count: 32),
                stakeAccountStateHash: String(repeating: "15", count: 32),
                stakeHistoryHash: String(repeating: "16", count: 32),
                stakeHistorySysvarAccountHash: String(repeating: "17", count: 32),
                sourceTrustAnchorHash: sourceTrustAnchorHash,
                consensusVerifierHash: consensusVerifierHash,
                messageInclusionVerifierHash: messageInclusionVerifierHash,
                finalityPolicyHash: finalityPolicyHash,
                sourceAdapterDeploymentReceiptHash: overrideSourceAdapterDeploymentReceiptHash ?? deploymentReceiptHash,
                solanaTowerReplayVerifierHash: overrideTowerVerifierHash ?? towerVerifierHash,
                solanaFullAccountsdbLatticeVerifierHash: overrideAccountsdbVerifierHash ?? accountsdbVerifierHash,
                solanaBankForkChoiceVerifierHash: overrideBankVerifierHash ?? bankVerifierHash,
                sourceVerifierMaterialHash: overrideSourceVerifierMaterialHash ?? sourceVerifierMaterialHash,
                sourceAdapterDeploymentHash: overrideSourceAdapterDeploymentHash ?? sourceAdapterDeploymentHash,
                fullLightClientGateHash: overrideFullLightClientGateHash ?? fullLightClientGateHash
            )
        }
        let input = auditInput()

        let requests = try buildSolanaSccpFullLightClientAuditProofRequests(input)
        var mismatchedAuditWitnessLtHash = accountsLtHash
        mismatchedAuditWitnessLtHash[0] = mismatchedAuditWitnessLtHash[0] ^ 0x01
        XCTAssertThrowsError(
            try buildSolanaSccpFullLightClientAuditProofRequests(
                auditInput(witness: witnessInput(accountsLtHash: mismatchedAuditWitnessLtHash))
            )
        ) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("accountsLtHash"))
        }
        let requestHashReusedTowerVerifierHash = requests.towerReplay.auditStatementHash
        let requestHashReusedDeploymentHash = try sccpSourceAdapterEngineDeploymentHash(
            sourceDomain: sccpDomainSolana,
            sourceTrustAnchorHash: sourceTrustAnchorHash,
            consensusVerifierHash: consensusVerifierHash,
            messageInclusionVerifierHash: messageInclusionVerifierHash,
            finalityPolicyHash: finalityPolicyHash,
            deploymentReceiptHash: deploymentReceiptHash,
            sourceStateVerifierHash: sourceStateVerifierHash,
            solanaTowerReplayVerifierHash: requestHashReusedTowerVerifierHash,
            solanaFullAccountsdbLatticeVerifierHash: accountsdbVerifierHash,
            solanaBankForkChoiceVerifierHash: bankVerifierHash
        )
        let requestHashReusedGateHash = try sccpSolanaFullLightClientGateHash(
            sourceTrustAnchorHash: sourceTrustAnchorHash,
            consensusVerifierHash: consensusVerifierHash,
            messageInclusionVerifierHash: messageInclusionVerifierHash,
            finalityPolicyHash: finalityPolicyHash,
            deploymentReceiptHash: deploymentReceiptHash,
            solanaTowerReplayVerifierHash: requestHashReusedTowerVerifierHash,
            solanaFullAccountsdbLatticeVerifierHash: accountsdbVerifierHash,
            solanaBankForkChoiceVerifierHash: bankVerifierHash,
            sourceStateVerifierHash: sourceStateVerifierHash
        )
        XCTAssertThrowsError(
            try buildSolanaSccpTowerReplayProofRequest(
                auditInput(
                    fullLightClientGateHash: requestHashReusedGateHash,
                    sourceAdapterDeploymentHash: requestHashReusedDeploymentHash,
                    towerVerifierHash: requestHashReusedTowerVerifierHash,
                    witness: witnessInput(sourceAdapterDeploymentHash: requestHashReusedDeploymentHash)
                )
            )
        ) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("verifierHash"))
        }
        let expectedTowerReplayColumns = [
            ["0x0100000000000000000000000000000000000000000000000000000000000000"],
            ["0x0300000000000000000000000000000000000000000000000000000000000000"],
            [Self.solanaMainnetGenesisPublicInput],
            ["0xe0c6130000000000000000000000000000000000000000000000000000000000"],
            ["0xb553931911947ab6caa4eba88d6aee62738b40f2e4d8d572e5e6616890abefbb"],
            ["0x2ead9384eaa2351b45a81bb22384a9bc9ed7c0793b06d0d3eb15424ef28929e3"],
            ["0xf0c76a74d7368857b724a8299f0851a30041acfbb03d6fc6bd4a6070358c093c"],
            ["0x9c33ee13a70d2c960e27e28680f7816b84bda7d6cb4888fb449f6407c87a2bbd"],
            ["0x3e0126e340dac71435abbb43b2df3bb5635568e8445326cd8723fef8a3dfd78f"],
            ["0xb1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1"],
            ["0x0300000000000000000000000000000000000000000000000000000000000000"],
            ["0xc1c6130000000000000000000000000000000000000000000000000000000000"],
            ["0xdfc6130000000000000000000000000000000000000000000000000000000000"],
            ["0x17a9f46bb57527c1579df8463067264c93125f1b5315fe3b537022809e76f3bc"],
            ["0xfc2832401bd6d624ab198e85a6ad1c889e09b393b3d16fff25a080c230c809dc"],
            ["0x922a426e06d6263986a0c9ff0f956f5429288c9c1310cb67fbaf30918de58b40"],
            ["0xaf75ee33d0fc85873b5302df026eaceddd40184c0f210a37968feea3b38d5ca0"],
            ["0xb114fd98978cd6d734a070976fb2e30a92110731bcc81ed2ace2698221aee727"],
            ["0x1313131313131313131313131313131313131313131313131313131313131313"],
            ["0x1414141414141414141414141414141414141414141414141414141414141414"],
            ["0x1515151515151515151515151515151515151515151515151515151515151515"],
            ["0x1616161616161616161616161616161616161616161616161616161616161616"],
            ["0x1717171717171717171717171717171717171717171717171717171717171717"],
            ["0x7777777777777777777777777777777777777777777777777777777777777777"],
        ]
        let expectedAccountsdbColumns = [
            ["0x0200000000000000000000000000000000000000000000000000000000000000"],
            ["0x0300000000000000000000000000000000000000000000000000000000000000"],
            [Self.solanaMainnetGenesisPublicInput],
            ["0xe0c6130000000000000000000000000000000000000000000000000000000000"],
            ["0xb553931911947ab6caa4eba88d6aee62738b40f2e4d8d572e5e6616890abefbb"],
            ["0x016d361178fe1ed787add1eb9b75b5cc37453995e24b0acd845bd977e1cc9df0"],
            ["0xf0c76a74d7368857b724a8299f0851a30041acfbb03d6fc6bd4a6070358c093c"],
            ["0x9c33ee13a70d2c960e27e28680f7816b84bda7d6cb4888fb449f6407c87a2bbd"],
            ["0x3e0126e340dac71435abbb43b2df3bb5635568e8445326cd8723fef8a3dfd78f"],
            ["0xc2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2"],
            ["0x0300000000000000000000000000000000000000000000000000000000000000"],
            ["0xc1c6130000000000000000000000000000000000000000000000000000000000"],
            ["0xdfc6130000000000000000000000000000000000000000000000000000000000"],
            ["0x17a9f46bb57527c1579df8463067264c93125f1b5315fe3b537022809e76f3bc"],
            ["0xfc2832401bd6d624ab198e85a6ad1c889e09b393b3d16fff25a080c230c809dc"],
            ["0x7777777777777777777777777777777777777777777777777777777777777777"],
            ["0xba606dacb76b0b03f395e6177a4a46cbe07f729678ab3a28f5ad8d7619cffc62"],
            ["0xc1b7c880344a2551d0842848f68b8519027e8b228a4c92c4e754141821d63810"],
            ["0x07270072f8b70b755ed491c1582b40050a484edd67752a8a0bbbd97aa175d4f9"],
            ["0x336bb79a5e96c331ddca555aedde346438de4ca1b227ae09f7faaa5e0e455be0"],
            ["0xfc2832401bd6d624ab198e85a6ad1c889e09b393b3d16fff25a080c230c809dc"],
        ]
        let expectedBankForkChoiceColumns = [
            ["0x0300000000000000000000000000000000000000000000000000000000000000"],
            ["0x0300000000000000000000000000000000000000000000000000000000000000"],
            [Self.solanaMainnetGenesisPublicInput],
            ["0xe0c6130000000000000000000000000000000000000000000000000000000000"],
            ["0xb553931911947ab6caa4eba88d6aee62738b40f2e4d8d572e5e6616890abefbb"],
            ["0x0c6a73bb4622acbb67c562c0a890237ca77619b33fececb645ee33b2028ed6a8"],
            ["0xf0c76a74d7368857b724a8299f0851a30041acfbb03d6fc6bd4a6070358c093c"],
            ["0x9c33ee13a70d2c960e27e28680f7816b84bda7d6cb4888fb449f6407c87a2bbd"],
            ["0x3e0126e340dac71435abbb43b2df3bb5635568e8445326cd8723fef8a3dfd78f"],
            ["0xd3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3d3"],
            ["0x0300000000000000000000000000000000000000000000000000000000000000"],
            ["0xc1c6130000000000000000000000000000000000000000000000000000000000"],
            ["0xdfc6130000000000000000000000000000000000000000000000000000000000"],
            ["0x17a9f46bb57527c1579df8463067264c93125f1b5315fe3b537022809e76f3bc"],
            ["0xfc2832401bd6d624ab198e85a6ad1c889e09b393b3d16fff25a080c230c809dc"],
            ["0xc0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0"],
            ["0x46bf9f58208a9c61b931640824eb13d636d3af5b0268cce866c958367bd6a451"],
            ["0x4242424242424242424242424242424242424242424242424242424242424242"],
            ["0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"],
            ["0x7777777777777777777777777777777777777777777777777777777777777777"],
            ["0xba606dacb76b0b03f395e6177a4a46cbe07f729678ab3a28f5ad8d7619cffc62"],
            ["0x0800000000000000000000000000000000000000000000000000000000000000"],
            ["0x1d2a51ef7c068fe46c9f588c252ce9cea8b66d87453bf73c9920005802e738bc"],
            ["0xb114fd98978cd6d734a070976fb2e30a92110731bcc81ed2ace2698221aee727"],
            ["0xaf75ee33d0fc85873b5302df026eaceddd40184c0f210a37968feea3b38d5ca0"],
        ]

        XCTAssertEqual(requests.towerReplay.circuitId, sccpSolanaTowerReplayOpenVerifyCircuitIdV1)
        XCTAssertEqual(
            requests.towerReplay.auditStatementHash,
            "0x2ead9384eaa2351b45a81bb22384a9bc9ed7c0793b06d0d3eb15424ef28929e3"
        )
        XCTAssertEqual(requests.towerReplay.statementBytes.count, 777)
        XCTAssertEqual(requests.towerReplay.publicInputColumns, expectedTowerReplayColumns)
        XCTAssertEqual(
            requests.fullAccountsdbLattice.circuitId,
            sccpSolanaFullAccountsdbLatticeOpenVerifyCircuitIdV1
        )
        XCTAssertEqual(
            requests.fullAccountsdbLattice.auditStatementHash,
            "0x016d361178fe1ed787add1eb9b75b5cc37453995e24b0acd845bd977e1cc9df0"
        )
        XCTAssertEqual(requests.fullAccountsdbLattice.statementBytes.count, 440)
        XCTAssertEqual(requests.fullAccountsdbLattice.publicInputColumns, expectedAccountsdbColumns)
        XCTAssertEqual(requests.bankForkChoice.circuitId, sccpSolanaBankForkChoiceOpenVerifyCircuitIdV1)
        XCTAssertEqual(
            requests.bankForkChoice.auditStatementHash,
            "0x0c6a73bb4622acbb67c562c0a890237ca77619b33fececb645ee33b2028ed6a8"
        )
        XCTAssertEqual(requests.bankForkChoice.statementBytes.count, 509)
        XCTAssertEqual(requests.bankForkChoice.publicInputColumns, expectedBankForkChoiceColumns)
        XCTAssertEqual(requests.bankForkChoice.publicInputColumns[19], [input.witness.accountInclusionRoot])
        XCTAssertTrue(
            String(decoding: requests.towerReplay.schemaDescriptor, as: UTF8.self)
                .contains("mainnet_genesis_hash")
        )
        XCTAssertEqual(
            requests.towerReplay.publicInputColumns[20],
            ["0x1515151515151515151515151515151515151515151515151515151515151515"]
        )
        XCTAssertEqual(
            requests.towerReplay.publicInputColumns[22],
            ["0x1717171717171717171717171717171717171717171717171717171717171717"]
        )
        XCTAssertEqual(requests.towerReplay.publicInputColumns[23], [input.witness.accountInclusionRoot])
        XCTAssertTrue(
            String(decoding: requests.towerReplay.schemaDescriptor, as: UTF8.self)
                .contains("stake_account_state_hash")
        )
        XCTAssertTrue(
            String(decoding: requests.towerReplay.schemaDescriptor, as: UTF8.self)
                .contains("stake_history_sysvar_account_hash")
        )
        XCTAssertTrue(
            String(decoding: requests.towerReplay.schemaDescriptor, as: UTF8.self)
                .contains("account_inclusion_root")
        )
        XCTAssertTrue(
            String(decoding: requests.bankForkChoice.schemaDescriptor, as: UTF8.self)
                .contains("account_inclusion_root")
        )
        XCTAssertTrue(
            String(decoding: requests.bankForkChoice.schemaDescriptor, as: UTF8.self)
                .contains("bank_hash_hard_fork_data_hash")
        )
        XCTAssertEqual(
            Set([
                requests.towerReplay.auditStatementHash,
                requests.fullAccountsdbLattice.auditStatementHash,
                requests.bankForkChoice.auditStatementHash,
            ]).count,
            3
        )
        XCTAssertEqual(requests.towerReplay.fullLightClientGateHash, fullLightClientGateHash)
        XCTAssertEqual(
            requests.towerReplay.finalityContextHash,
            try solanaSccpFullLightClientAuditFinalityContextHash(input)
        )
        XCTAssertEqual(
            requests.towerReplay.voteMessageHash,
            try solanaSccpFullLightClientAuditVoteMessageHash(input)
        )
        XCTAssertEqual(
            requests.towerReplay.accountsLtHashProofHash,
            try solanaSccpAccountsLtHashProofHash(input.accountsLtHashProof)
        )
        for request in [
            requests.towerReplay,
            requests.fullAccountsdbLattice,
            requests.bankForkChoice,
        ] {
            let proofCapsule = try wrapSolanaSccpSourceStateVerificationProof(
                proofBytes: Data([9, 8, 7]),
                request: request
            )
            XCTAssertEqual(proofCapsule.version, request.version)
            XCTAssertEqual(proofCapsule.proofFamily, request.proofFamily)
            XCTAssertEqual(proofCapsule.circuitId, request.circuitId)
            XCTAssertEqual(proofCapsule.proofBytes, Data([9, 8, 7]))
            XCTAssertEqual(proofCapsule.proofBase64, "CQgH")
            XCTAssertGreaterThan(
                try canonicalSolanaSccpSourceStateVerificationProofBytes(proofCapsule).count,
                0
            )
            XCTAssertThrowsError(try solanaSccpAccountsLtHashProofHash(proofCapsule))
            var exposedProofBytes = proofCapsule.proofBytes
            exposedProofBytes[0] = 1
            XCTAssertEqual(proofCapsule.proofBytes, Data([9, 8, 7]))
            XCTAssertEqual(proofCapsule.proofBase64, "CQgH")
        }
        var wrongAuditGenesisColumns = requests.bankForkChoice.publicInputColumns
        wrongAuditGenesisColumns[2][0] = "0x" + String(repeating: "aa", count: 32)
        let wrongAuditGenesisRequest = SolanaSccpFullLightClientAuditProofRequest(
            version: requests.bankForkChoice.version,
            proofFamily: requests.bankForkChoice.proofFamily,
            circuitId: requests.bankForkChoice.circuitId,
            parameterSet: requests.bankForkChoice.parameterSet,
            role: requests.bankForkChoice.role,
            roleCode: requests.bankForkChoice.roleCode,
            sourceDomain: requests.bankForkChoice.sourceDomain,
            finalizedSlot: requests.bankForkChoice.finalizedSlot,
            verifierId: requests.bankForkChoice.verifierId,
            verifierHash: requests.bankForkChoice.verifierHash,
            sourceStateVerifierId: requests.bankForkChoice.sourceStateVerifierId,
            sourceStateVerifierHash: requests.bankForkChoice.sourceStateVerifierHash,
            sourceVerifierMaterialHash: requests.bankForkChoice.sourceVerifierMaterialHash,
            sourceAdapterDeploymentHash: requests.bankForkChoice.sourceAdapterDeploymentHash,
            fullLightClientGateHash: requests.bankForkChoice.fullLightClientGateHash,
            finalityContextHash: requests.bankForkChoice.finalityContextHash,
            voteMessageHash: requests.bankForkChoice.voteMessageHash,
            accountsLtHashProofHash: requests.bankForkChoice.accountsLtHashProofHash,
            auditStatementHash: requests.bankForkChoice.auditStatementHash,
            statementBytes: requests.bankForkChoice.statementBytes,
            verificationContextBytes: requests.bankForkChoice.verificationContextBytes,
            schemaDescriptor: requests.bankForkChoice.schemaDescriptor,
            publicInputColumns: wrongAuditGenesisColumns,
            fastpqPublicInputs: requests.bankForkChoice.fastpqPublicInputs,
            fastpqTransitions: requests.bankForkChoice.fastpqTransitions
        )
        XCTAssertThrowsError(
            try wrapSolanaSccpSourceStateVerificationProof(
                proofBytes: Data([9, 8, 7]),
                request: wrongAuditGenesisRequest
            )
        ) { error in
            XCTAssertEqual(
                error as? SolanaSccpProverError,
                .invalidString("request.publicInputColumns.mainnet_genesis_hash")
            )
        }
        var wrongAuditStatementColumns = requests.towerReplay.publicInputColumns
        wrongAuditStatementColumns[5][0] = "0x" + String(repeating: "cc", count: 32)
        let wrongAuditStatementRequest = SolanaSccpFullLightClientAuditProofRequest(
            version: requests.towerReplay.version,
            proofFamily: requests.towerReplay.proofFamily,
            circuitId: requests.towerReplay.circuitId,
            parameterSet: requests.towerReplay.parameterSet,
            role: requests.towerReplay.role,
            roleCode: requests.towerReplay.roleCode,
            sourceDomain: requests.towerReplay.sourceDomain,
            finalizedSlot: requests.towerReplay.finalizedSlot,
            verifierId: requests.towerReplay.verifierId,
            verifierHash: requests.towerReplay.verifierHash,
            sourceStateVerifierId: requests.towerReplay.sourceStateVerifierId,
            sourceStateVerifierHash: requests.towerReplay.sourceStateVerifierHash,
            sourceVerifierMaterialHash: requests.towerReplay.sourceVerifierMaterialHash,
            sourceAdapterDeploymentHash: requests.towerReplay.sourceAdapterDeploymentHash,
            fullLightClientGateHash: requests.towerReplay.fullLightClientGateHash,
            finalityContextHash: requests.towerReplay.finalityContextHash,
            voteMessageHash: requests.towerReplay.voteMessageHash,
            accountsLtHashProofHash: requests.towerReplay.accountsLtHashProofHash,
            auditStatementHash: requests.towerReplay.auditStatementHash,
            statementBytes: requests.towerReplay.statementBytes,
            verificationContextBytes: requests.towerReplay.verificationContextBytes,
            schemaDescriptor: requests.towerReplay.schemaDescriptor,
            publicInputColumns: wrongAuditStatementColumns,
            fastpqPublicInputs: requests.towerReplay.fastpqPublicInputs,
            fastpqTransitions: requests.towerReplay.fastpqTransitions
        )
        XCTAssertThrowsError(
            try wrapSolanaSccpSourceStateVerificationProof(
                proofBytes: Data([9, 8, 7]),
                request: wrongAuditStatementRequest
            )
        ) { error in
            XCTAssertEqual(
                error as? SolanaSccpProverError,
                .invalidString("request.publicInputColumns.audit_statement_hash")
            )
        }
        let staleAuditHashRequest = Self.fullLightClientAuditRequest(
            requests.towerReplay,
            auditStatementHash: "0x" + String(repeating: "cc", count: 32)
        )
        XCTAssertThrowsError(
            try wrapSolanaSccpSourceStateVerificationProof(
                proofBytes: Data([9, 8, 7]),
                request: staleAuditHashRequest
            )
        ) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("request.auditStatementHash"))
        }
        let wrongAuditDsidInputs = SolanaSccpFullLightClientAuditFastpqPublicInputs(
            dsid: "0x" + String(repeating: "00", count: 16),
            slot: requests.towerReplay.fastpqPublicInputs.slot,
            oldRoot: requests.towerReplay.fastpqPublicInputs.oldRoot,
            newRoot: requests.towerReplay.fastpqPublicInputs.newRoot,
            permRoot: requests.towerReplay.fastpqPublicInputs.permRoot,
            txSetHash: requests.towerReplay.fastpqPublicInputs.txSetHash
        )
        let wrongAuditDsidRequest = Self.fullLightClientAuditRequest(
            requests.towerReplay,
            fastpqPublicInputs: wrongAuditDsidInputs
        )
        XCTAssertThrowsError(
            try wrapSolanaSccpSourceStateVerificationProof(
                proofBytes: Data([9, 8, 7]),
                request: wrongAuditDsidRequest
            )
        ) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("request.fastpqPublicInputs.dsid"))
        }
        let wrongAuditTxInputs = SolanaSccpFullLightClientAuditFastpqPublicInputs(
            dsid: requests.towerReplay.fastpqPublicInputs.dsid,
            slot: requests.towerReplay.fastpqPublicInputs.slot,
            oldRoot: requests.towerReplay.fastpqPublicInputs.oldRoot,
            newRoot: requests.towerReplay.fastpqPublicInputs.newRoot,
            permRoot: requests.towerReplay.fastpqPublicInputs.permRoot,
            txSetHash: "0x" + String(repeating: "cc", count: 32)
        )
        let wrongAuditTxRequest = Self.fullLightClientAuditRequest(
            requests.towerReplay,
            fastpqPublicInputs: wrongAuditTxInputs
        )
        XCTAssertThrowsError(
            try wrapSolanaSccpSourceStateVerificationProof(
                proofBytes: Data([9, 8, 7]),
                request: wrongAuditTxRequest
            )
        ) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("request.fastpqPublicInputs.txSetHash"))
        }
        let reusedSourceStateVerifierRequest = Self.fullLightClientAuditRequest(
            requests.towerReplay,
            verifierHash: requests.towerReplay.sourceStateVerifierHash
        )
        XCTAssertThrowsError(
            try wrapSolanaSccpSourceStateVerificationProof(
                proofBytes: Data([9, 8, 7]),
                request: reusedSourceStateVerifierRequest
            )
        ) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("request.verifierHash"))
        }
        var wrongAuditTransitions = requests.towerReplay.fastpqTransitions
        wrongAuditTransitions[0] = SolanaSccpFullLightClientAuditFastpqTransition(
            key: wrongAuditTransitions[0].key,
            operation: wrongAuditTransitions[0].operation,
            oldValue: wrongAuditTransitions[0].oldValue,
            newValue: Data([0])
        )
        let wrongAuditTransitionRequest = SolanaSccpFullLightClientAuditProofRequest(
            version: requests.towerReplay.version,
            proofFamily: requests.towerReplay.proofFamily,
            circuitId: requests.towerReplay.circuitId,
            parameterSet: requests.towerReplay.parameterSet,
            role: requests.towerReplay.role,
            roleCode: requests.towerReplay.roleCode,
            sourceDomain: requests.towerReplay.sourceDomain,
            finalizedSlot: requests.towerReplay.finalizedSlot,
            verifierId: requests.towerReplay.verifierId,
            verifierHash: requests.towerReplay.verifierHash,
            sourceStateVerifierId: requests.towerReplay.sourceStateVerifierId,
            sourceStateVerifierHash: requests.towerReplay.sourceStateVerifierHash,
            sourceVerifierMaterialHash: requests.towerReplay.sourceVerifierMaterialHash,
            sourceAdapterDeploymentHash: requests.towerReplay.sourceAdapterDeploymentHash,
            fullLightClientGateHash: requests.towerReplay.fullLightClientGateHash,
            finalityContextHash: requests.towerReplay.finalityContextHash,
            voteMessageHash: requests.towerReplay.voteMessageHash,
            accountsLtHashProofHash: requests.towerReplay.accountsLtHashProofHash,
            auditStatementHash: requests.towerReplay.auditStatementHash,
            statementBytes: requests.towerReplay.statementBytes,
            verificationContextBytes: requests.towerReplay.verificationContextBytes,
            schemaDescriptor: requests.towerReplay.schemaDescriptor,
            publicInputColumns: requests.towerReplay.publicInputColumns,
            fastpqPublicInputs: requests.towerReplay.fastpqPublicInputs,
            fastpqTransitions: wrongAuditTransitions
        )
        XCTAssertThrowsError(
            try wrapSolanaSccpSourceStateVerificationProof(
                proofBytes: Data([9, 8, 7]),
                request: wrongAuditTransitionRequest
            )
        ) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("request.fastpqTransitions"))
        }
        var wrongAuditOldValueTransitions = requests.towerReplay.fastpqTransitions
        wrongAuditOldValueTransitions[0] = SolanaSccpFullLightClientAuditFastpqTransition(
            key: wrongAuditOldValueTransitions[0].key,
            operation: wrongAuditOldValueTransitions[0].operation,
            oldValue: Data([0]),
            newValue: wrongAuditOldValueTransitions[0].newValue
        )
        let wrongAuditOldValueRequest = SolanaSccpFullLightClientAuditProofRequest(
            version: requests.towerReplay.version,
            proofFamily: requests.towerReplay.proofFamily,
            circuitId: requests.towerReplay.circuitId,
            parameterSet: requests.towerReplay.parameterSet,
            role: requests.towerReplay.role,
            roleCode: requests.towerReplay.roleCode,
            sourceDomain: requests.towerReplay.sourceDomain,
            finalizedSlot: requests.towerReplay.finalizedSlot,
            verifierId: requests.towerReplay.verifierId,
            verifierHash: requests.towerReplay.verifierHash,
            sourceStateVerifierId: requests.towerReplay.sourceStateVerifierId,
            sourceStateVerifierHash: requests.towerReplay.sourceStateVerifierHash,
            sourceVerifierMaterialHash: requests.towerReplay.sourceVerifierMaterialHash,
            sourceAdapterDeploymentHash: requests.towerReplay.sourceAdapterDeploymentHash,
            fullLightClientGateHash: requests.towerReplay.fullLightClientGateHash,
            finalityContextHash: requests.towerReplay.finalityContextHash,
            voteMessageHash: requests.towerReplay.voteMessageHash,
            accountsLtHashProofHash: requests.towerReplay.accountsLtHashProofHash,
            auditStatementHash: requests.towerReplay.auditStatementHash,
            statementBytes: requests.towerReplay.statementBytes,
            verificationContextBytes: requests.towerReplay.verificationContextBytes,
            schemaDescriptor: requests.towerReplay.schemaDescriptor,
            publicInputColumns: requests.towerReplay.publicInputColumns,
            fastpqPublicInputs: requests.towerReplay.fastpqPublicInputs,
            fastpqTransitions: wrongAuditOldValueTransitions
        )
        XCTAssertThrowsError(
            try wrapSolanaSccpSourceStateVerificationProof(
                proofBytes: Data([9, 8, 7]),
                request: wrongAuditOldValueRequest
            )
        ) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("request.fastpqTransitions"))
        }
        let malformedAuditRequest = SolanaSccpFullLightClientAuditProofRequest(
            version: requests.towerReplay.version,
            proofFamily: requests.towerReplay.proofFamily,
            circuitId: requests.towerReplay.circuitId,
            parameterSet: requests.towerReplay.parameterSet,
            role: requests.towerReplay.role,
            roleCode: requests.towerReplay.roleCode,
            sourceDomain: requests.towerReplay.sourceDomain,
            finalizedSlot: requests.towerReplay.finalizedSlot,
            verifierId: requests.towerReplay.verifierId,
            verifierHash: requests.towerReplay.verifierHash,
            sourceStateVerifierId: requests.towerReplay.sourceStateVerifierId,
            sourceStateVerifierHash: requests.towerReplay.sourceStateVerifierHash,
            sourceVerifierMaterialHash: requests.towerReplay.sourceVerifierMaterialHash,
            sourceAdapterDeploymentHash: requests.towerReplay.sourceAdapterDeploymentHash,
            fullLightClientGateHash: requests.towerReplay.fullLightClientGateHash,
            finalityContextHash: requests.towerReplay.finalityContextHash,
            voteMessageHash: requests.towerReplay.voteMessageHash,
            accountsLtHashProofHash: requests.towerReplay.accountsLtHashProofHash,
            auditStatementHash: requests.towerReplay.auditStatementHash,
            statementBytes: requests.towerReplay.statementBytes,
            verificationContextBytes: requests.towerReplay.verificationContextBytes,
            schemaDescriptor: requests.towerReplay.schemaDescriptor,
            publicInputColumns: [],
            fastpqPublicInputs: requests.towerReplay.fastpqPublicInputs,
            fastpqTransitions: requests.towerReplay.fastpqTransitions
        )
        XCTAssertThrowsError(
            try wrapSolanaSccpSourceStateVerificationProof(
                proofBytes: Data([9, 8, 7]),
                request: malformedAuditRequest
            )
        ) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("request.publicInputColumns"))
        }
        var rejectedAuditCallbackRan = false
        let guardingAuditProver = SolanaSccpSourceStateProver(
            fullLightClientAuditProveFunction: { _ in
                rejectedAuditCallbackRan = true
                return Data([9, 8, 7])
            }
        )
        do {
            _ = try await guardingAuditProver.proveFullLightClientAudit(request: malformedAuditRequest)
            XCTFail("expected malformed full-light audit request to fail before callback")
        } catch {
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("request.publicInputColumns"))
            XCTAssertFalse(rejectedAuditCallbackRan)
        }
        XCTAssertThrowsError(
            try solanaSccpAccountsLtHashProofHash(
                SolanaSccpSourceStateVerificationProof(
                    circuitId: input.accountsLtHashProof.circuitId,
                    proofBytes: Data(repeating: 0, count: 3)
                )
            )
        ) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .allZeroProof)
        }
        XCTAssertThrowsError(
            try canonicalSolanaSccpSourceStateVerificationProofBytes(
                SolanaSccpSourceStateVerificationProof(
                    version: 0,
                    circuitId: input.accountsLtHashProof.circuitId,
                    proofBytes: Data([1, 2, 3])
                )
            )
        ) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("accountsLtHashProof"))
        }
        XCTAssertNotNil(
            requests.fullAccountsdbLattice.schemaDescriptor.range(
                of: Data("full_light_client_gate_hash".utf8)
            )
        )
        var exposedAuditStatement = requests.towerReplay.statementBytes
        exposedAuditStatement[exposedAuditStatement.startIndex] =
            exposedAuditStatement[exposedAuditStatement.startIndex] == 0 ? 1 : 0
        XCTAssertEqual(
            requests.towerReplay.statementBytes,
            try canonicalSolanaSccpFullLightClientAuditStatementBytes(input, role: .towerReplay)
        )
        var exposedAuditSchema = requests.fullAccountsdbLattice.schemaDescriptor
        exposedAuditSchema[exposedAuditSchema.startIndex] =
            exposedAuditSchema[exposedAuditSchema.startIndex] == 0 ? 1 : 0
        XCTAssertEqual(
            requests.fullAccountsdbLattice.schemaDescriptor,
            try solanaSccpFullLightClientAuditOpenVerifySchemaDescriptor(input, role: .fullAccountsdbLattice)
        )
        var seenRoles: [String] = []
        let sourceStateProver = SolanaSccpSourceStateProver(
            fullLightClientAuditProveFunction: { request in
                seenRoles.append(request.role)
                return Data([9, 8, 7])
            }
        )
        let linkedProofs = try await sourceStateProver.proveFullLightClientAudit(input)
        XCTAssertEqual(seenRoles, ["tower_replay", "full_accountsdb_lattice", "bank_fork_choice"])
        XCTAssertEqual(
            linkedProofs.towerReplay.circuitId,
            sccpSolanaTowerReplayOpenVerifyCircuitIdV1
        )
        XCTAssertEqual(
            linkedProofs.fullAccountsdbLattice.circuitId,
            sccpSolanaFullAccountsdbLatticeOpenVerifyCircuitIdV1
        )
        XCTAssertEqual(
            linkedProofs.bankForkChoice.circuitId,
            sccpSolanaBankForkChoiceOpenVerifyCircuitIdV1
        )
        XCTAssertEqual(linkedProofs.bankForkChoice.proofBase64, "CQgH")
        do {
            _ = try await SolanaSccpSourceStateProver().proveFullLightClientAudit(input)
            XCTFail("expected missing source-state prover")
        } catch {
            XCTAssertEqual(error as? SolanaSccpProverError, .localProverUnavailable)
        }
        XCTAssertTrue(requests.bankForkChoice.fastpqTransitions.allSatisfy { $0.key.hasPrefix("0x") })
        XCTAssertThrowsError(
            try buildSolanaSccpFullLightClientAuditProofRequests(
                auditInput(fullLightClientGateHash: "0x" + String(repeating: "ab", count: 32))
            )
        ) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidHex32("fullLightClientGateHash"))
        }
        XCTAssertThrowsError(
            try buildSolanaSccpFullLightClientAuditProofRequests(
                auditInput(sourceVerifierMaterialHash: "0x" + String(repeating: "ab", count: 32))
            )
        ) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidHex32("sourceVerifierMaterialHash"))
        }
        XCTAssertThrowsError(
            try buildSolanaSccpFullLightClientAuditProofRequests(
                auditInput(sourceAdapterDeploymentReceiptHash: "0x" + String(repeating: "ab", count: 32))
            )
        ) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidHex32("sourceAdapterDeploymentReceiptHash"))
        }
        XCTAssertThrowsError(
            try buildSolanaSccpFullLightClientAuditProofRequests(
                auditInput(witness: witnessInput(
                    sourceAdapterDeploymentHash: "0x" + String(repeating: "ab", count: 32)
                ))
            )
        ) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .sourceAdapterDeploymentBindingMismatch)
        }
        XCTAssertThrowsError(
            try buildSolanaSccpFullLightClientAuditProofRequests(
                auditInput(witness: witnessInput(
                    sourceAdapterDeploymentReceiptHash: "0x" + String(repeating: "ab", count: 32)
                ))
            )
        ) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidHex32("sourceAdapterDeploymentReceiptHash"))
        }
        XCTAssertThrowsError(
            try buildSolanaSccpFullLightClientAuditProofRequests(
                auditInput(towerVerifierHash: accountsdbVerifierHash)
            )
        ) { error in
            XCTAssertEqual(
                error as? SolanaSccpProverError,
                .invalidString("solanaFullLightClientAuditVerifierHashes")
            )
        }
        XCTAssertThrowsError(
            try buildSolanaSccpFullLightClientAuditProofRequests(
                auditInput(towerVerifierHash: sourceStateVerifierHash)
            )
        ) { error in
            XCTAssertEqual(
                error as? SolanaSccpProverError,
                .invalidString("solanaFullLightClientAuditVerifierHashes")
            )
        }
        XCTAssertThrowsError(
            try buildSolanaSccpFullLightClientAuditProofRequests(
                auditInput(towerVerifierHash: sourceTrustAnchorHash)
            )
        ) { error in
            XCTAssertEqual(
                error as? SolanaSccpProverError,
                .invalidString("solanaFullLightClientAuditVerifierHashes")
            )
        }
        XCTAssertThrowsError(
            try buildSolanaSccpFullLightClientAuditProofRequests(
                auditInput(
                    towerVerifierHash: try sccpSourceAdapterVerifierVkHash(
                        sourceDomain: sccpDomainSolana
                    )
                )
            )
        ) { error in
            XCTAssertEqual(
                error as? SolanaSccpProverError,
                .invalidString("solanaFullLightClientAuditVerifierHashes")
            )
        }
        XCTAssertThrowsError(
            try buildSolanaSccpFullLightClientAuditProofRequests(
                auditInput(
                    sourceAdapterDeploymentReceiptHash: towerVerifierHash,
                    towerVerifierHash: towerVerifierHash,
                    witness: witnessInput(sourceAdapterDeploymentReceiptHash: towerVerifierHash)
                )
            )
        ) { error in
            XCTAssertEqual(
                error as? SolanaSccpProverError,
                .invalidString("solanaFullLightClientAuditVerifierHashes")
            )
        }
        XCTAssertThrowsError(
            try buildSolanaSccpFullLightClientAuditProofRequests(
                auditInput(
                    towerVerifierHash:
                        "0x113bdb7601d84f2098daec386346a7123857d181b3ac5bd23df50fa9e1b2cbe3"
                )
            )
        ) { error in
            XCTAssertEqual(
                error as? SolanaSccpProverError,
                .invalidString("solanaFullLightClientAuditVerifierHashes")
            )
        }
    }

    func testBuildsSolanaVoteAndStakeAccountDataHashes() throws {
        let towerVoteSlots = (11...41).map(UInt64.init)

        XCTAssertEqual(
            try canonicalSolanaSccpVoteAccountDataBytes(
                nodePubkey: Data(repeating: 0x51, count: 32),
                authorizedVoter: Data(repeating: 0x61, count: 32),
                authorizedWithdrawer: Data(repeating: 0x71, count: 32),
                inflationRewardsCollector: Data(repeating: 0x81, count: 32),
                blockRevenueCollector: Data(repeating: 0x51, count: 32),
                inflationRewardsCommissionBps: 700,
                blockRevenueCommissionBps: 10_000,
                pendingDelegatorRewards: 123,
                rootSlot: 10,
                towerVoteSlots: towerVoteSlots
            ).count,
            457
        )
        let voteHash = try solanaSccpVoteAccountDataHash(
            nodePubkey: Data(repeating: 0x51, count: 32),
            authorizedVoter: Data(repeating: 0x61, count: 32),
            authorizedWithdrawer: Data(repeating: 0x71, count: 32),
            inflationRewardsCollector: Data(repeating: 0x81, count: 32),
            blockRevenueCollector: Data(repeating: 0x51, count: 32),
            inflationRewardsCommissionBps: 700,
            blockRevenueCommissionBps: 10_000,
            pendingDelegatorRewards: 123,
            rootSlot: 10,
            towerVoteSlots: towerVoteSlots
        )
        XCTAssertTrue(voteHash.range(of: #"^0x[0-9a-f]{64}$"#, options: .regularExpression) != nil)
        XCTAssertNotEqual(
            voteHash,
            try solanaSccpVoteAccountDataHash(
                nodePubkey: Data(repeating: 0x51, count: 32),
                authorizedVoter: Data(repeating: 0x62, count: 32),
                authorizedWithdrawer: Data(repeating: 0x71, count: 32),
                inflationRewardsCollector: Data(repeating: 0x81, count: 32),
                blockRevenueCollector: Data(repeating: 0x51, count: 32),
                inflationRewardsCommissionBps: 700,
                blockRevenueCommissionBps: 10_000,
                pendingDelegatorRewards: 123,
                rootSlot: 10,
                towerVoteSlots: towerVoteSlots
            )
        )
        XCTAssertThrowsError(try solanaSccpVoteAccountDataHash(
            nodePubkey: Data(repeating: 0x51, count: 32),
            authorizedVoter: Data(repeating: 0x61, count: 32),
            authorizedWithdrawer: Data(repeating: 0x71, count: 32),
            inflationRewardsCollector: Data(repeating: 0x81, count: 32),
            blockRevenueCollector: Data(repeating: 0x51, count: 32),
            inflationRewardsCommissionBps: 700,
            blockRevenueCommissionBps: 10_000,
            pendingDelegatorRewards: 123,
            rootSlot: 10,
            towerVoteSlots: [UInt64(10)] + Array(towerVoteSlots.dropFirst())
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("towerVoteSlots[0]"))
        }

        XCTAssertEqual(
            try canonicalSolanaSccpStakeAccountDataBytes(
                staker: Data(repeating: 0x81, count: 32),
                withdrawer: Data(repeating: 0x91, count: 32),
                voterPubkey: Data(repeating: 0xa1, count: 32),
                delegatedStake: 1_000,
                activationEpoch: 2,
                deactivationEpoch: 9,
                warmupCooldownRateBytes: Data([0x0a, 0xd7, 0xa3, 0x70, 0x3d, 0x0a, 0xb7, 0x3f]),
                creditsObserved: 123,
                stakeFlags: 1
            ).count,
            154
        )
        let stakeHash = try solanaSccpStakeAccountDataHash(
            staker: Data(repeating: 0x81, count: 32),
            withdrawer: Data(repeating: 0x91, count: 32),
            voterPubkey: Data(repeating: 0xa1, count: 32),
            delegatedStake: 1_000,
            activationEpoch: 2,
            deactivationEpoch: 9,
            warmupCooldownRateBytes: Data([0x0a, 0xd7, 0xa3, 0x70, 0x3d, 0x0a, 0xb7, 0x3f]),
            creditsObserved: 123,
            stakeFlags: 1
        )
        XCTAssertTrue(stakeHash.range(of: #"^0x[0-9a-f]{64}$"#, options: .regularExpression) != nil)
        XCTAssertNotEqual(
            stakeHash,
            try solanaSccpStakeAccountDataHash(
                staker: Data(repeating: 0x81, count: 32),
                withdrawer: Data(repeating: 0x91, count: 32),
                voterPubkey: Data(repeating: 0xa2, count: 32),
                delegatedStake: 1_000,
                activationEpoch: 2,
                deactivationEpoch: 9,
                warmupCooldownRateBytes: Data([0x0a, 0xd7, 0xa3, 0x70, 0x3d, 0x0a, 0xb7, 0x3f]),
                creditsObserved: 123,
                stakeFlags: 1
            )
        )
        XCTAssertTrue(try solanaSccpStakeAccountDataHash(
            staker: Data(repeating: 0x81, count: 32),
            withdrawer: Data(repeating: 0x91, count: 32),
            voterPubkey: Data(repeating: 0xa1, count: 32),
            delegatedStake: 1_000,
            activationEpoch: 2,
            deactivationEpoch: 9,
            warmupCooldownRateBytes: Data([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xd0, 0x3f]),
            creditsObserved: 123,
            stakeFlags: 1
        ).range(of: #"^0x[0-9a-f]{64}$"#, options: .regularExpression) != nil)
        XCTAssertThrowsError(try solanaSccpStakeAccountDataHash(
                staker: Data(repeating: 0x81, count: 32),
                withdrawer: Data(repeating: 0x91, count: 32),
                voterPubkey: Data(repeating: 0xa1, count: 32),
                delegatedStake: 1_000,
                activationEpoch: 2,
                deactivationEpoch: 9,
                warmupCooldownRateBytes: Data(repeating: 0, count: 8),
                creditsObserved: 123,
                stakeFlags: 1
            )
        ) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("warmupCooldownRateBytes"))
        }
        XCTAssertNotEqual(
            stakeHash,
            try solanaSccpStakeAccountDataHash(
                staker: Data(repeating: 0x81, count: 32),
                withdrawer: Data(repeating: 0x91, count: 32),
                voterPubkey: Data(repeating: 0xa1, count: 32),
                delegatedStake: 1_000,
                activationEpoch: 2,
                deactivationEpoch: 9,
                warmupCooldownRateBytes: Data([0x0a, 0xd7, 0xa3, 0x70, 0x3d, 0x0a, 0xb7, 0x3f]),
                creditsObserved: 123,
                stakeFlags: 0
            )
        )
        XCTAssertThrowsError(try solanaSccpStakeAccountDataHash(
            staker: Data(repeating: 0x81, count: 32),
            withdrawer: Data(repeating: 0x91, count: 32),
            voterPubkey: Data(repeating: 0xa1, count: 32),
            delegatedStake: 1_000,
            activationEpoch: 2,
            deactivationEpoch: 2,
            warmupCooldownRateBytes: Data([0x0a, 0xd7, 0xa3, 0x70, 0x3d, 0x0a, 0xb7, 0x3f]),
            creditsObserved: 123,
            stakeFlags: 1
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("deactivationEpoch"))
        }
        XCTAssertThrowsError(try solanaSccpStakeAccountDataHash(
            staker: Data(repeating: 0x81, count: 32),
            withdrawer: Data(repeating: 0x91, count: 32),
            voterPubkey: Data(repeating: 0xa1, count: 32),
            delegatedStake: 1_000,
            activationEpoch: 2,
            deactivationEpoch: 9,
            warmupCooldownRateBytes: Data([0x0a, 0xd7, 0xa3, 0x70, 0x3d, 0x0a, 0xb7, 0x3f]),
            creditsObserved: 123,
            stakeFlags: 2
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("stakeFlags"))
        }
        XCTAssertThrowsError(try solanaSccpStakeAccountDataHash(
            staker: Data(repeating: 0x81, count: 32),
            withdrawer: Data(repeating: 0x91, count: 32),
            voterPubkey: Data(repeating: 0xa1, count: 32),
            delegatedStake: 1_000,
            activationEpoch: 2,
            deactivationEpoch: 9,
            warmupCooldownRateBytes: Data(repeating: 0, count: 7),
            creditsObserved: 123,
            stakeFlags: 1
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("warmupCooldownRateBytes"))
        }
    }

    func testBuildsSolanaVoteAccountDataHashFromRawVoteState() throws {
        let voteAccountAddress = Data(repeating: 0x81, count: 32)
        let rawV3 = Self.sampleSolanaVoteStateAccount(hasLatency: true)
        let parsed = try solanaSccpVoteAccountDataFromRawVoteState(
            rawData: rawV3,
            epoch: 3,
            voteAccountAddress: voteAccountAddress
        )
        XCTAssertEqual(parsed.nodePubkey, Data(repeating: 0x51, count: 32))
        XCTAssertEqual(parsed.authorizedVoter, Data(repeating: 0x61, count: 32))
        XCTAssertEqual(parsed.authorizedWithdrawer, Data(repeating: 0x71, count: 32))
        XCTAssertEqual(parsed.inflationRewardsCollector, voteAccountAddress)
        XCTAssertEqual(parsed.blockRevenueCollector, Data(repeating: 0x51, count: 32))
        XCTAssertEqual(parsed.inflationRewardsCommissionBps, 700)
        XCTAssertEqual(parsed.blockRevenueCommissionBps, 10_000)
        XCTAssertEqual(parsed.pendingDelegatorRewards, 0)
        XCTAssertEqual(parsed.blsPubkeyCompressed, Data())
        XCTAssertEqual(parsed.rootSlot, 10)
        XCTAssertEqual(parsed.towerVoteSlots, (11...41).map(UInt64.init))
        XCTAssertEqual(
            try solanaSccpVoteAccountDataHashFromRawVoteState(
                rawData: rawV3,
                epoch: 3,
                voteAccountAddress: voteAccountAddress
            ),
            try solanaSccpVoteAccountDataHash(
                nodePubkey: parsed.nodePubkey,
                authorizedVoter: parsed.authorizedVoter,
                authorizedWithdrawer: parsed.authorizedWithdrawer,
                inflationRewardsCollector: parsed.inflationRewardsCollector,
                blockRevenueCollector: parsed.blockRevenueCollector,
                inflationRewardsCommissionBps: parsed.inflationRewardsCommissionBps,
                blockRevenueCommissionBps: parsed.blockRevenueCommissionBps,
                pendingDelegatorRewards: parsed.pendingDelegatorRewards,
                blsPubkeyCompressed: parsed.blsPubkeyCompressed,
                rootSlot: parsed.rootSlot,
                towerVoteSlots: parsed.towerVoteSlots
            )
        )

        let rawV1 = Self.sampleSolanaVoteStateAccount(hasLatency: false)
        XCTAssertEqual(
            try solanaSccpVoteAccountDataFromRawVoteState(
                rawData: rawV1,
                epoch: 3,
                voteAccountAddress: voteAccountAddress
            ).towerVoteSlots,
            parsed.towerVoteSlots
        )

        let rawV4 = Self.sampleSolanaVoteStateV4Account(withBls: true)
        let parsedV4 = try solanaSccpVoteAccountDataFromRawVoteState(
            rawData: rawV4,
            epoch: 3,
            voteAccountAddress: voteAccountAddress
        )
        XCTAssertEqual(parsedV4.inflationRewardsCollector, Data(repeating: 0x81, count: 32))
        XCTAssertEqual(parsedV4.blockRevenueCollector, Data(repeating: 0x91, count: 32))
        XCTAssertEqual(parsedV4.inflationRewardsCommissionBps, 1_234)
        XCTAssertEqual(parsedV4.blockRevenueCommissionBps, 9_876)
        XCTAssertEqual(parsedV4.pendingDelegatorRewards, 456)
        XCTAssertEqual(parsedV4.blsPubkeyCompressed, Data(repeating: 0xa5, count: 48))
        let v4InflationCommissionBpsOffset = 4 + (4 * 32)
        var excessiveInflationCommissionV4 = rawV4
        excessiveInflationCommissionV4[
            v4InflationCommissionBpsOffset..<(v4InflationCommissionBpsOffset + 2)
        ] = Data([0x11, 0x27])
        XCTAssertThrowsError(try solanaSccpVoteAccountDataFromRawVoteState(
            rawData: excessiveInflationCommissionV4,
            epoch: 3,
            voteAccountAddress: voteAccountAddress
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("inflationRewardsCommissionBps"))
        }
        var excessiveBlockCommissionV4 = rawV4
        excessiveBlockCommissionV4[
            (v4InflationCommissionBpsOffset + 2)..<(v4InflationCommissionBpsOffset + 4)
        ] = Data([0x11, 0x27])
        XCTAssertThrowsError(try solanaSccpVoteAccountDataFromRawVoteState(
            rawData: excessiveBlockCommissionV4,
            epoch: 3,
            voteAccountAddress: voteAccountAddress
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("blockRevenueCommissionBps"))
        }
        XCTAssertThrowsError(try solanaSccpVoteAccountDataHash(
            nodePubkey: parsedV4.nodePubkey,
            authorizedVoter: parsedV4.authorizedVoter,
            authorizedWithdrawer: parsedV4.authorizedWithdrawer,
            inflationRewardsCollector: parsedV4.inflationRewardsCollector,
            blockRevenueCollector: parsedV4.blockRevenueCollector,
            inflationRewardsCommissionBps: parsedV4.inflationRewardsCommissionBps,
            blockRevenueCommissionBps: parsedV4.blockRevenueCommissionBps,
            pendingDelegatorRewards: parsedV4.pendingDelegatorRewards,
            blsPubkeyCompressed: Data(repeating: 0, count: 48),
            rootSlot: parsedV4.rootSlot,
            towerVoteSlots: parsedV4.towerVoteSlots
        ))
        var allZeroBlsV4 = rawV4
        let v4BlsPubkeyOffset = 4 + (4 * 32) + 2 + 2 + 8 + 1
        allZeroBlsV4[v4BlsPubkeyOffset..<(v4BlsPubkeyOffset + 48)] =
            Data(repeating: 0, count: 48)
        XCTAssertThrowsError(try solanaSccpVoteAccountDataFromRawVoteState(
            rawData: allZeroBlsV4,
            epoch: 3,
            voteAccountAddress: voteAccountAddress
        ))
        let parsedV4FourAuthorized = try solanaSccpVoteAccountDataFromRawVoteState(
            rawData: Self.sampleSolanaVoteStateV4Account(withBls: true, authorizedVoterCount: 4),
            epoch: 3,
            voteAccountAddress: voteAccountAddress
        )
        XCTAssertEqual(parsedV4FourAuthorized.authorizedVoter, Data(repeating: 0x62, count: 32))

        var wrongVoteCount = rawV3
        wrongVoteCount[(4 + 32 + 32 + 1)..<(4 + 32 + 32 + 1 + 8)] = Data([30, 0, 0, 0, 0, 0, 0, 0])
        XCTAssertThrowsError(try solanaSccpVoteAccountDataFromRawVoteState(
            rawData: wrongVoteCount,
            epoch: 3,
            voteAccountAddress: voteAccountAddress
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("towerVoteSlots"))
        }

        let voteEntryOffset = 4 + 32 + 32 + 1 + 8
        let firstVoteSlotOffset = voteEntryOffset + 1
        let firstConfirmationOffset = firstVoteSlotOffset + 8
        let secondVoteSlotOffset = voteEntryOffset + (1 + 8 + 4) + 1
        let rootOptionOffset = voteEntryOffset + (31 * (1 + 8 + 4))

        var wrongConfirmationCount = rawV3
        wrongConfirmationCount[firstConfirmationOffset..<(firstConfirmationOffset + 4)] = Data([30, 0, 0, 0])
        XCTAssertThrowsError(try solanaSccpVoteAccountDataFromRawVoteState(
            rawData: wrongConfirmationCount,
            epoch: 3,
            voteAccountAddress: voteAccountAddress
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("towerVoteSlots[0]"))
        }

        var repeatedVoteSlot = rawV3
        repeatedVoteSlot[secondVoteSlotOffset..<(secondVoteSlotOffset + 8)] = Data([11, 0, 0, 0, 0, 0, 0, 0])
        XCTAssertThrowsError(try solanaSccpVoteAccountDataFromRawVoteState(
            rawData: repeatedVoteSlot,
            epoch: 3,
            voteAccountAddress: voteAccountAddress
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("towerVoteSlots[1]"))
        }

        var noRoot = rawV3
        noRoot[rootOptionOffset] = 0
        XCTAssertThrowsError(try solanaSccpVoteAccountDataFromRawVoteState(
            rawData: noRoot,
            epoch: 3,
            voteAccountAddress: voteAccountAddress
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("rootSlot"))
        }

        var rootOverlapsVoteStack = rawV3
        rootOverlapsVoteStack[(rootOptionOffset + 1)..<(rootOptionOffset + 9)] = Data([11, 0, 0, 0, 0, 0, 0, 0])
        XCTAssertThrowsError(try solanaSccpVoteAccountDataFromRawVoteState(
            rawData: rootOverlapsVoteStack,
            epoch: 3,
            voteAccountAddress: voteAccountAddress
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("towerVoteSlots[0]"))
        }

        var badPriorVoters = rawV3
        let priorVotersOffset = rootOptionOffset + 1 + 8 + 8 + (2 * (8 + 32))
        var zeroPriorVoterWithEpochBounds = rawV3
        zeroPriorVoterWithEpochBounds[(priorVotersOffset + 32)..<(priorVotersOffset + 40)] =
            Data([1, 0, 0, 0, 0, 0, 0, 0])
        XCTAssertThrowsError(try solanaSccpVoteAccountDataFromRawVoteState(
            rawData: zeroPriorVoterWithEpochBounds,
            epoch: 3,
            voteAccountAddress: voteAccountAddress
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("priorVoters[0]"))
        }
        badPriorVoters[priorVotersOffset + (32 * (32 + 8 + 8)) + 8] = 2
        XCTAssertThrowsError(try solanaSccpVoteAccountDataFromRawVoteState(
            rawData: badPriorVoters,
            epoch: 3,
            voteAccountAddress: voteAccountAddress
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("priorVoters"))
        }

        var tooManyEpochCredits = rawV4
        let v4AuthorizedVotersOffset = 4 + 32 + 32 + 32 + 32 + 2 + 2 + 8 + 1 + 48 + 8
            + (31 * (1 + 8 + 4)) + 1 + 8
        var zeroFutureAuthorizedVoter = Self.sampleSolanaVoteStateV4Account(
            withBls: true,
            authorizedVoterCount: 4
        )
        let fourthAuthorizedVoterKeyOffset = v4AuthorizedVotersOffset + 8 + (3 * (8 + 32)) + 8
        zeroFutureAuthorizedVoter[fourthAuthorizedVoterKeyOffset..<(fourthAuthorizedVoterKeyOffset + 32)] =
            Data(repeating: 0, count: 32)
        XCTAssertThrowsError(try solanaSccpVoteAccountDataFromRawVoteState(
            rawData: zeroFutureAuthorizedVoter,
            epoch: 3,
            voteAccountAddress: voteAccountAddress
        )) { error in
            XCTAssertEqual(
                error as? SolanaSccpProverError,
                .invalidString("authorizedVoters[3].authorizedVoter")
            )
        }
        let tooManyV4AuthorizedVoters = Self.sampleSolanaVoteStateV4Account(
            withBls: true,
            authorizedVoterCount: 5
        )
        XCTAssertThrowsError(try solanaSccpVoteAccountDataFromRawVoteState(
            rawData: tooManyV4AuthorizedVoters,
            epoch: 3,
            voteAccountAddress: voteAccountAddress
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("authorizedVoters"))
        }

        let v4EpochCreditsOffset = v4AuthorizedVotersOffset + 8 + (2 * (8 + 32))
        tooManyEpochCredits[v4EpochCreditsOffset..<(v4EpochCreditsOffset + 8)] = Data([65, 0, 0, 0, 0, 0, 0, 0])
        XCTAssertThrowsError(try solanaSccpVoteAccountDataFromRawVoteState(
            rawData: tooManyEpochCredits,
            epoch: 3,
            voteAccountAddress: voteAccountAddress
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("epochCredits"))
        }

        let v3EpochCreditsOffset = priorVotersOffset + (32 * (32 + 8 + 8)) + 8 + 1
        var futureEpochCredit = rawV3
        futureEpochCredit[v3EpochCreditsOffset..<(v3EpochCreditsOffset + 8)] = Data([1, 0, 0, 0, 0, 0, 0, 0])
        futureEpochCredit[(v3EpochCreditsOffset + 8)..<(v3EpochCreditsOffset + 16)] = Data([4, 0, 0, 0, 0, 0, 0, 0])
        futureEpochCredit[(v3EpochCreditsOffset + 16)..<(v3EpochCreditsOffset + 24)] = Data([1, 0, 0, 0, 0, 0, 0, 0])
        XCTAssertThrowsError(try solanaSccpVoteAccountDataFromRawVoteState(
            rawData: futureEpochCredit,
            epoch: 3,
            voteAccountAddress: voteAccountAddress
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("epochCredits"))
        }

        let lastTimestampSlotOffset = v3EpochCreditsOffset + 8
        var futureLastTimestampSlot = rawV3
        futureLastTimestampSlot[lastTimestampSlotOffset..<(lastTimestampSlotOffset + 8)] = Data([42, 0, 0, 0, 0, 0, 0, 0])
        XCTAssertThrowsError(try solanaSccpVoteAccountDataFromRawVoteState(
            rawData: futureLastTimestampSlot,
            epoch: 3,
            voteAccountAddress: voteAccountAddress
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("lastTimestamp"))
        }

        var negativeLastTimestamp = rawV3
        negativeLastTimestamp[lastTimestampSlotOffset..<(lastTimestampSlotOffset + 8)] = Data([41, 0, 0, 0, 0, 0, 0, 0])
        negativeLastTimestamp[(lastTimestampSlotOffset + 8)..<(lastTimestampSlotOffset + 16)] =
            Data(repeating: 0xff, count: 8)
        XCTAssertThrowsError(try solanaSccpVoteAccountDataFromRawVoteState(
            rawData: negativeLastTimestamp,
            epoch: 3,
            voteAccountAddress: voteAccountAddress
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("lastTimestamp"))
        }

        var nonzeroPadding = rawV3
        nonzeroPadding[nonzeroPadding.count - 1] = 1
        XCTAssertThrowsError(try solanaSccpVoteAccountDataFromRawVoteState(
            rawData: nonzeroPadding,
            epoch: 3,
            voteAccountAddress: voteAccountAddress
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("rawDataPadding"))
        }

        XCTAssertThrowsError(try solanaSccpVoteAccountDataFromRawVoteState(
            rawData: rawV3,
            epoch: 0,
            voteAccountAddress: voteAccountAddress
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("authorizedVoters"))
        }
    }

    func testBuildsSolanaStakeAccountDataHashFromRawStakeStateV2() throws {
        let raw = Self.sampleSolanaStakeStateV2StakeAccount()
        let parsed = try solanaSccpStakeAccountDataFromRawStakeStateV2(rawData: raw)
        XCTAssertEqual(parsed.staker, Data(repeating: 0x81, count: 32))
        XCTAssertEqual(parsed.withdrawer, Data(repeating: 0x91, count: 32))
        XCTAssertEqual(parsed.voterPubkey, Data(repeating: 0xa1, count: 32))
        XCTAssertEqual(parsed.delegatedStake, 1_000)
        XCTAssertEqual(parsed.activationEpoch, 2)
        XCTAssertEqual(parsed.deactivationEpoch, 9)
        XCTAssertEqual(parsed.warmupCooldownRateBytes, Data([0x0a, 0xd7, 0xa3, 0x70, 0x3d, 0x0a, 0xb7, 0x3f]))
        XCTAssertEqual(parsed.creditsObserved, 123)
        XCTAssertEqual(parsed.stakeFlags, 1)
        XCTAssertEqual(
            try solanaSccpStakeAccountDataHashFromRawStakeStateV2(rawData: raw),
            try solanaSccpStakeAccountDataHash(
                staker: parsed.staker,
                withdrawer: parsed.withdrawer,
                voterPubkey: parsed.voterPubkey,
                delegatedStake: parsed.delegatedStake,
                activationEpoch: parsed.activationEpoch,
                deactivationEpoch: parsed.deactivationEpoch,
                warmupCooldownRateBytes: parsed.warmupCooldownRateBytes,
                creditsObserved: parsed.creditsObserved,
                stakeFlags: parsed.stakeFlags
            )
        )

        var wrongVariant = raw
        wrongVariant[0..<4] = Data([1, 0, 0, 0])
        XCTAssertThrowsError(try solanaSccpStakeAccountDataFromRawStakeStateV2(rawData: wrongVariant)) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("rawData"))
        }
        XCTAssertThrowsError(try solanaSccpStakeAccountDataFromRawStakeStateV2(rawData: raw.dropLast())) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("rawData"))
        }

        var hiddenPadding = raw
        hiddenPadding[197] = 1
        XCTAssertThrowsError(try solanaSccpStakeAccountDataFromRawStakeStateV2(rawData: hiddenPadding)) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("rawData"))
        }

        var unknownFlags = raw
        unknownFlags[196] = 2
        XCTAssertThrowsError(try solanaSccpStakeAccountDataFromRawStakeStateV2(rawData: unknownFlags)) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("stakeFlags"))
        }

        var zeroVoter = raw
        zeroVoter[124..<156] = Data(repeating: 0, count: 32)
        XCTAssertThrowsError(try solanaSccpStakeAccountDataFromRawStakeStateV2(rawData: zeroVoter)) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("voterPubkey"))
        }

        var zeroDelegation = raw
        zeroDelegation[156..<164] = Data(repeating: 0, count: 8)
        XCTAssertThrowsError(try solanaSccpStakeAccountDataFromRawStakeStateV2(rawData: zeroDelegation)) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("delegatedStake"))
        }

        var legacyWarmupCooldownRate = raw
        legacyWarmupCooldownRate[180..<188] = Data([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xd0, 0x3f])
        XCTAssertEqual(
            try solanaSccpStakeAccountDataFromRawStakeStateV2(rawData: legacyWarmupCooldownRate)
                .warmupCooldownRateBytes,
            Data([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xd0, 0x3f])
        )

        var zeroWarmupCooldownRate = raw
        zeroWarmupCooldownRate[180..<188] = Data(repeating: 0, count: 8)
        XCTAssertThrowsError(try solanaSccpStakeAccountDataFromRawStakeStateV2(rawData: zeroWarmupCooldownRate)) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("warmupCooldownRateBytes"))
        }

        var invalidEpochOrder = raw
        invalidEpochOrder[172..<180] = Data([2, 0, 0, 0, 0, 0, 0, 0])
        XCTAssertThrowsError(try solanaSccpStakeAccountDataFromRawStakeStateV2(rawData: invalidEpochOrder)) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("deactivationEpoch"))
        }
    }

    func testBuildsSolanaStakeAccountStateHashForFinalityContext() throws {
        let validatorPublicKeys = [
            Data(repeating: 0x11, count: 32),
            Data(repeating: 0x22, count: 32),
        ]
        let validatorStakes: [UInt64] = [1, 2]
        let activationEpochs: [UInt64] = [0, 2]
        let deactivationEpochs: [UInt64] = [UInt64.max, 9]
        let voteAccounts = [
            Data(repeating: 0x33, count: 32),
            Data(repeating: 0x44, count: 32),
        ]
        let stakeAccounts = [
            Data(repeating: 0x55, count: 32),
            Data(repeating: 0x66, count: 32),
        ]
        let voteAccountHashes = [
            Data(repeating: 0x77, count: 32),
            Data(repeating: 0x88, count: 32),
        ]
        let stakeAccountHashes = [
            Data(repeating: 0x99, count: 32),
            Data(repeating: 0xaa, count: 32),
        ]

        XCTAssertEqual(
            try canonicalSolanaSccpStakeAccountStateBytes(
                epoch: 3,
                validatorPublicKeys: validatorPublicKeys,
                validatorStakes: validatorStakes,
                validatorActivationEpochs: activationEpochs,
                validatorDeactivationEpochs: deactivationEpochs,
                validatorVoteAccountAddresses: voteAccounts,
                validatorStakeAccountAddresses: stakeAccounts,
                validatorVoteAccountHashes: voteAccountHashes,
                validatorStakeAccountHashes: stakeAccountHashes
            ).count,
            437
        )
        XCTAssertEqual(
            try solanaSccpStakeAccountStateHash(
                epoch: 3,
                validatorPublicKeys: validatorPublicKeys,
                validatorStakes: validatorStakes,
                validatorActivationEpochs: activationEpochs,
                validatorDeactivationEpochs: deactivationEpochs,
                validatorVoteAccountAddresses: voteAccounts,
                validatorStakeAccountAddresses: stakeAccounts,
                validatorVoteAccountHashes: voteAccountHashes,
                validatorStakeAccountHashes: stakeAccountHashes
            ),
            "0x34f6086dd8c1770770802be17b833ed7c973fdaa002c866c0462c33d6938f5b5"
        )
        XCTAssertThrowsError(try solanaSccpStakeAccountStateHash(
            epoch: 3,
            validatorPublicKeys: validatorPublicKeys,
            validatorStakes: validatorStakes,
            validatorActivationEpochs: activationEpochs,
            validatorDeactivationEpochs: deactivationEpochs,
            validatorVoteAccountAddresses: [Data(repeating: 0x33, count: 32)],
            validatorStakeAccountAddresses: stakeAccounts,
            validatorVoteAccountHashes: voteAccountHashes,
            validatorStakeAccountHashes: stakeAccountHashes
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("validatorVoteAccountAddresses"))
        }
        XCTAssertThrowsError(try solanaSccpStakeAccountStateHash(
            epoch: 3,
            validatorPublicKeys: validatorPublicKeys,
            validatorStakes: validatorStakes,
            validatorActivationEpochs: activationEpochs,
            validatorDeactivationEpochs: deactivationEpochs,
            validatorVoteAccountAddresses: [Data(repeating: 0x33, count: 32), Data(repeating: 0x33, count: 32)],
            validatorStakeAccountAddresses: stakeAccounts,
            validatorVoteAccountHashes: voteAccountHashes,
            validatorStakeAccountHashes: stakeAccountHashes
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("validatorVoteAccountAddresses"))
        }
        XCTAssertThrowsError(try solanaSccpStakeAccountStateHash(
            epoch: 3,
            validatorPublicKeys: validatorPublicKeys,
            validatorStakes: validatorStakes,
            validatorActivationEpochs: activationEpochs,
            validatorDeactivationEpochs: deactivationEpochs,
            validatorVoteAccountAddresses: voteAccounts,
            validatorStakeAccountAddresses: [Data(repeating: 0x55, count: 32), Data(repeating: 0x44, count: 32)],
            validatorVoteAccountHashes: voteAccountHashes,
            validatorStakeAccountHashes: stakeAccountHashes
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("validatorStakeAccountAddresses[1]"))
        }
        XCTAssertThrowsError(try solanaSccpStakeAccountStateHash(
            epoch: 3,
            validatorPublicKeys: validatorPublicKeys,
            validatorStakes: validatorStakes,
            validatorActivationEpochs: activationEpochs,
            validatorDeactivationEpochs: deactivationEpochs,
            validatorVoteAccountAddresses: [Data(repeating: 0x66, count: 32), Data(repeating: 0x44, count: 32)],
            validatorStakeAccountAddresses: stakeAccounts,
            validatorVoteAccountHashes: voteAccountHashes,
            validatorStakeAccountHashes: stakeAccountHashes
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("validatorStakeAccountAddresses"))
        }
        XCTAssertThrowsError(try solanaSccpStakeAccountStateHash(
            epoch: 3,
            validatorPublicKeys: validatorPublicKeys,
            validatorStakes: validatorStakes,
            validatorActivationEpochs: activationEpochs,
            validatorDeactivationEpochs: deactivationEpochs,
            validatorVoteAccountAddresses: voteAccounts,
            validatorStakeAccountAddresses: stakeAccounts,
            validatorVoteAccountHashes: [Data(repeating: 0x77, count: 32), Data(repeating: 0x00, count: 32)],
            validatorStakeAccountHashes: stakeAccountHashes
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidHex32("validatorVoteAccountHashes[1]"))
        }
    }

    func testBuildsSolanaStakeHistoryHashForFinalityContext() throws {
        let validatorPublicKeys = [
            Data(repeating: 0x11, count: 32),
            Data(repeating: 0x22, count: 32),
        ]
        let validatorEffectiveStakes: [UInt64] = [1, 2]
        let validatorDelegatedStakes: [UInt64] = [1, 3]
        let activationEpochs: [UInt64] = [0, 2]
        let deactivationEpochs: [UInt64] = [UInt64.max, 9]
        let voteAccounts = [
            Data(repeating: 0x33, count: 32),
            Data(repeating: 0x44, count: 32),
        ]
        let stakeAccounts = [
            Data(repeating: 0x55, count: 32),
            Data(repeating: 0x66, count: 32),
        ]
        let voteAccountHashes = [
            Data(repeating: 0x77, count: 32),
            Data(repeating: 0x88, count: 32),
        ]
        let stakeAccountHashes = [
            Data(repeating: 0x99, count: 32),
            Data(repeating: 0xaa, count: 32),
        ]
        let stakeHistoryEntries = [
            SolanaSccpStakeHistoryEntry(epoch: 2, effective: 23, activating: 3, deactivating: 0),
            SolanaSccpStakeHistoryEntry(epoch: 3, effective: 3, activating: 1, deactivating: 0),
        ]

        XCTAssertEqual(
            try canonicalSolanaSccpStakeHistoryBytes(
                epoch: 3,
                validatorPublicKeys: validatorPublicKeys,
                validatorEffectiveStakes: validatorEffectiveStakes,
                validatorDelegatedStakes: validatorDelegatedStakes,
                validatorActivationEpochs: activationEpochs,
                validatorDeactivationEpochs: deactivationEpochs,
                validatorVoteAccountAddresses: voteAccounts,
                validatorStakeAccountAddresses: stakeAccounts,
                validatorVoteAccountHashes: voteAccountHashes,
                validatorStakeAccountHashes: stakeAccountHashes,
                stakeHistoryEntries: stakeHistoryEntries
            ).count,
            249
        )
        XCTAssertEqual(
            try solanaSccpStakeHistoryHash(
                epoch: 3,
                validatorPublicKeys: validatorPublicKeys,
                validatorEffectiveStakes: validatorEffectiveStakes,
                validatorDelegatedStakes: validatorDelegatedStakes,
                validatorActivationEpochs: activationEpochs,
                validatorDeactivationEpochs: deactivationEpochs,
                validatorVoteAccountAddresses: voteAccounts,
                validatorStakeAccountAddresses: stakeAccounts,
                validatorVoteAccountHashes: voteAccountHashes,
                validatorStakeAccountHashes: stakeAccountHashes,
                stakeHistoryEntries: stakeHistoryEntries
            ),
            "0xd75957eec3cf9f5b88076c8dc18e81c5debd627adfbed7e03e35443bcc4d14b6"
        )
        XCTAssertThrowsError(try solanaSccpStakeHistoryHash(
            epoch: 3,
            validatorPublicKeys: validatorPublicKeys,
            validatorEffectiveStakes: validatorEffectiveStakes,
            validatorDelegatedStakes: [0, 3],
            validatorActivationEpochs: activationEpochs,
            validatorDeactivationEpochs: deactivationEpochs,
            validatorVoteAccountAddresses: voteAccounts,
            validatorStakeAccountAddresses: stakeAccounts,
            validatorVoteAccountHashes: voteAccountHashes,
            validatorStakeAccountHashes: stakeAccountHashes,
            stakeHistoryEntries: stakeHistoryEntries
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("validatorDelegatedStakes[0]"))
        }
        XCTAssertThrowsError(try solanaSccpStakeHistoryHash(
            epoch: 3,
            validatorPublicKeys: validatorPublicKeys,
            validatorEffectiveStakes: [1, 1],
            validatorDelegatedStakes: validatorDelegatedStakes,
            validatorActivationEpochs: activationEpochs,
            validatorDeactivationEpochs: deactivationEpochs,
            validatorVoteAccountAddresses: voteAccounts,
            validatorStakeAccountAddresses: stakeAccounts,
            validatorVoteAccountHashes: voteAccountHashes,
            validatorStakeAccountHashes: stakeAccountHashes,
            stakeHistoryEntries: stakeHistoryEntries
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("validatorEffectiveStakes[1]"))
        }
        XCTAssertThrowsError(try solanaSccpStakeHistoryHash(
            epoch: 3,
            validatorPublicKeys: validatorPublicKeys,
            validatorEffectiveStakes: validatorEffectiveStakes,
            validatorDelegatedStakes: validatorDelegatedStakes,
            validatorActivationEpochs: activationEpochs,
            validatorDeactivationEpochs: deactivationEpochs,
            validatorVoteAccountAddresses: voteAccounts,
            validatorStakeAccountAddresses: stakeAccounts,
            validatorVoteAccountHashes: voteAccountHashes,
            validatorStakeAccountHashes: stakeAccountHashes,
            stakeHistoryEntries: [
                stakeHistoryEntries[0],
                SolanaSccpStakeHistoryEntry(epoch: 3, effective: 4, activating: 1, deactivating: 0),
            ]
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("stakeHistoryEntries"))
        }
        XCTAssertThrowsError(try solanaSccpStakeHistoryHash(
            epoch: 3,
            validatorPublicKeys: validatorPublicKeys,
            validatorEffectiveStakes: validatorEffectiveStakes,
            validatorDelegatedStakes: validatorDelegatedStakes,
            validatorActivationEpochs: activationEpochs,
            validatorDeactivationEpochs: deactivationEpochs,
            validatorVoteAccountAddresses: voteAccounts,
            validatorStakeAccountAddresses: stakeAccounts,
            validatorVoteAccountHashes: voteAccountHashes,
            validatorStakeAccountHashes: stakeAccountHashes,
            stakeHistoryEntries: Array(stakeHistoryEntries.prefix(1))
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("stakeHistoryEntries"))
        }
    }

    func testBuildsSolanaStakeHistorySysvarDataHash() throws {
        let stakeHistoryEntries = [
            SolanaSccpStakeHistoryEntry(epoch: 2, effective: 10, activating: 3, deactivating: 1),
            SolanaSccpStakeHistoryEntry(epoch: 3, effective: 12, activating: 0, deactivating: 0),
        ]

        XCTAssertEqual(sccpSolanaSysvarProgramId.count, 32)
        XCTAssertEqual(sccpSolanaStakeHistorySysvarId.count, 32)
        let canonical = try canonicalSolanaSccpStakeHistorySysvarDataBytes(
            stakeHistoryEntries: stakeHistoryEntries
        )
        XCTAssertEqual(canonical.count, 72)
        let dataHash = try solanaSccpStakeHistorySysvarDataHash(
            stakeHistoryEntries: stakeHistoryEntries
        )
        XCTAssertTrue(dataHash.range(of: #"^0x[0-9a-f]{64}$"#, options: .regularExpression) != nil)
        XCTAssertEqual(
            try solanaSccpStakeHistorySysvarDataHashFromRawData(rawData: canonical),
            dataHash
        )
        XCTAssertNotEqual(
            dataHash,
            try solanaSccpStakeHistorySysvarDataHash(
                stakeHistoryEntries: [
                    stakeHistoryEntries[0],
                    SolanaSccpStakeHistoryEntry(
                        epoch: 3,
                        effective: 13,
                        activating: 0,
                        deactivating: 0
                    ),
                ]
            )
        )
        XCTAssertThrowsError(try solanaSccpStakeHistorySysvarDataHash(
            stakeHistoryEntries: Array(stakeHistoryEntries.reversed())
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("stakeHistoryEntries"))
        }
        XCTAssertThrowsError(
            try solanaSccpStakeHistorySysvarDataHashFromRawData(rawData: Data(canonical.prefix(9)))
        ) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("rawData"))
        }
        var wrongCount = canonical
        wrongCount[0..<8] = Data([3, 0, 0, 0, 0, 0, 0, 0])
        XCTAssertThrowsError(
            try solanaSccpStakeHistorySysvarDataHashFromRawData(rawData: wrongCount)
        ) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("rawData"))
        }
        var ascendingRaw = canonical
        let newestEntry = Data(canonical[8..<40])
        let oldestEntry = Data(canonical[40..<72])
        ascendingRaw[8..<40] = oldestEntry
        ascendingRaw[40..<72] = newestEntry
        XCTAssertThrowsError(
            try solanaSccpStakeHistorySysvarDataHashFromRawData(rawData: ascendingRaw)
        ) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("rawDataOrder"))
        }
    }

    func testBuildsSolanaTowerLockoutHashForFinalityContext() throws {
        let finalizedSlot: UInt64 = 1_296_096
        let rootedSlot: UInt64 = 1_296_065
        let parentSlot: UInt64 = 1_296_095
        let parentBankHash = "0x" + String(repeating: "33", count: 32)

        XCTAssertEqual(sccpSolanaTowerLockoutConfirmationDepth, 32)
        XCTAssertEqual(sccpSolanaTowerVoteStackDepth, 31)
        XCTAssertEqual(
            try canonicalSolanaSccpTowerLockoutBytes(
                finalizedSlot: finalizedSlot,
                rootedSlot: rootedSlot,
                parentSlot: parentSlot,
                parentBankHash: parentBankHash
            ).count,
            73
        )
        XCTAssertTrue(
            try solanaSccpTowerLockoutHash(
                finalizedSlot: finalizedSlot,
                rootedSlot: rootedSlot,
                parentSlot: parentSlot,
                parentBankHash: parentBankHash
            ).hasPrefix("0x")
        )
        XCTAssertThrowsError(try solanaSccpTowerLockoutHash(
            epoch: 4,
            finalizedSlot: finalizedSlot,
            rootedSlot: rootedSlot,
            parentSlot: parentSlot,
            parentBankHash: parentBankHash
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("epoch"))
        }
        XCTAssertThrowsError(try solanaSccpTowerLockoutHash(
            finalizedSlot: finalizedSlot,
            rootedSlot: rootedSlot + 1,
            parentSlot: parentSlot,
            parentBankHash: parentBankHash
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("rootedSlot"))
        }
        XCTAssertThrowsError(try solanaSccpTowerLockoutHash(
            finalizedSlot: finalizedSlot,
            rootedSlot: rootedSlot,
            parentSlot: parentSlot - 1,
            parentBankHash: parentBankHash
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("parentSlot"))
        }
        XCTAssertThrowsError(try solanaSccpTowerLockoutHash(
            finalizedSlot: finalizedSlot,
            rootedSlot: rootedSlot,
            parentSlot: parentSlot,
            parentBankHash: "0x" + String(repeating: "00", count: 32)
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidHex32("parentBankHash"))
        }
    }

    func testBuildsSolanaTowerReplayHashForFinalityContext() throws {
        let finalizedSlot: UInt64 = 1_296_096
        let rootedSlot: UInt64 = 1_296_065
        let parentSlot: UInt64 = 1_296_095
        let bankForkHash = "0x" + String(repeating: "a5", count: 32)
        let towerVoteSlots = Array((rootedSlot + 1)...finalizedSlot)

        XCTAssertEqual(
            try canonicalSolanaSccpTowerReplayBytes(
                finalizedSlot: finalizedSlot,
                rootedSlot: rootedSlot,
                parentSlot: parentSlot,
                bankForkHash: bankForkHash,
                towerVoteSlots: towerVoteSlots
            ).count,
            573
        )
        XCTAssertTrue(
            try solanaSccpTowerReplayHash(
                finalizedSlot: finalizedSlot,
                rootedSlot: rootedSlot,
                parentSlot: parentSlot,
                bankForkHash: bankForkHash,
                towerVoteSlots: towerVoteSlots
            ).hasPrefix("0x")
        )
        XCTAssertNotEqual(
            try solanaSccpTowerReplayHash(
                finalizedSlot: finalizedSlot,
                rootedSlot: rootedSlot,
                parentSlot: parentSlot,
                bankForkHash: bankForkHash,
                towerVoteSlots: towerVoteSlots
            ),
            try solanaSccpTowerReplayHash(
                finalizedSlot: finalizedSlot,
                rootedSlot: rootedSlot,
                parentSlot: parentSlot,
                bankForkHash: "0x" + String(repeating: "a6", count: 32),
                towerVoteSlots: towerVoteSlots
            )
        )
        XCTAssertThrowsError(try solanaSccpTowerReplayHash(
            finalizedSlot: finalizedSlot,
            rootedSlot: rootedSlot,
            parentSlot: parentSlot,
            bankForkHash: "0x" + String(repeating: "00", count: 32),
            towerVoteSlots: towerVoteSlots
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidHex32("bankForkHash"))
        }
        XCTAssertThrowsError(try solanaSccpTowerReplayHash(
            epoch: 4,
            finalizedSlot: finalizedSlot,
            rootedSlot: rootedSlot,
            parentSlot: parentSlot,
            bankForkHash: bankForkHash,
            towerVoteSlots: towerVoteSlots
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("epoch"))
        }
        XCTAssertThrowsError(try solanaSccpTowerReplayHash(
            finalizedSlot: finalizedSlot,
            rootedSlot: rootedSlot,
            parentSlot: parentSlot,
            bankForkHash: bankForkHash,
            towerVoteSlots: Array(towerVoteSlots.dropFirst())
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("towerVoteSlots"))
        }
        var unsortedVoteSlots = towerVoteSlots
        unsortedVoteSlots.swapAt(0, 1)
        XCTAssertThrowsError(try solanaSccpTowerReplayHash(
            finalizedSlot: finalizedSlot,
            rootedSlot: rootedSlot,
            parentSlot: parentSlot,
            bankForkHash: bankForkHash,
            towerVoteSlots: unsortedVoteSlots
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("towerVoteSlots"))
        }
        var wrongLastVoteSlots = towerVoteSlots
        wrongLastVoteSlots[wrongLastVoteSlots.count - 1] -= 1
        XCTAssertThrowsError(try solanaSccpTowerReplayHash(
            finalizedSlot: finalizedSlot,
            rootedSlot: rootedSlot,
            parentSlot: parentSlot,
            bankForkHash: bankForkHash,
            towerVoteSlots: wrongLastVoteSlots
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("towerVoteSlots"))
        }
    }

    func testBuildsSolanaBankForkHashForFinalityContext() throws {
        let finalizedSlot: UInt64 = 1_296_096
        let parentSlot: UInt64 = 1_296_095
        let parentBankHash = "0x" + String(repeating: "33", count: 32)
        let bankSignatureCount: UInt64 = 8
        let blockhash = "0x" + String(repeating: "55", count: 32)
        let accountsLtHash = Data(repeating: 0x99, count: 2_048)
        let bankHash = try solanaSccpAgaveBankHash(
            parentBankHash: parentBankHash,
            bankSignatureCount: bankSignatureCount,
            blockhash: blockhash,
            accountsLtHash: accountsLtHash
        )
        let transactionStatusRoot = "0x" + String(repeating: "66", count: 32)
        let accountInclusionRoot = "0x" + String(repeating: "77", count: 32)
        let accountsLtHashChecksum = try solanaSccpAccountsLtHashChecksum(accountsLtHash)

        XCTAssertEqual(
            try canonicalSolanaSccpBankForkBytes(
                finalizedSlot: finalizedSlot,
                parentSlot: parentSlot,
                bankSignatureCount: bankSignatureCount,
                parentBankHash: parentBankHash,
                bankHash: bankHash,
                blockhash: blockhash,
                accountsLtHash: accountsLtHash,
                transactionStatusRoot: transactionStatusRoot,
                accountInclusionRoot: accountInclusionRoot,
                accountsLtHashChecksum: accountsLtHashChecksum
            ).count,
            229
        )
        XCTAssertEqual(
            try solanaSccpBankForkHash(
                finalizedSlot: finalizedSlot,
                parentSlot: parentSlot,
                bankSignatureCount: bankSignatureCount,
                parentBankHash: parentBankHash,
                bankHash: bankHash,
                blockhash: blockhash,
                accountsLtHash: accountsLtHash,
                transactionStatusRoot: transactionStatusRoot,
                accountInclusionRoot: accountInclusionRoot,
                accountsLtHashChecksum: accountsLtHashChecksum
            ),
            "0x8c496fb25a4499947e454a84f638211a84445748bc5242fbb6fb511edd82e531"
        )
        XCTAssertEqual(
            try solanaSccpBankForkHash(
                epoch: 3,
                finalizedSlot: finalizedSlot,
                parentSlot: parentSlot,
                bankSignatureCount: bankSignatureCount,
                parentBankHash: parentBankHash,
                bankHash: bankHash,
                blockhash: blockhash,
                accountsLtHash: accountsLtHash,
                transactionStatusRoot: transactionStatusRoot,
                accountInclusionRoot: accountInclusionRoot,
                accountsLtHashChecksum: accountsLtHashChecksum
            ),
            try solanaSccpBankForkHash(
                finalizedSlot: finalizedSlot,
                parentSlot: parentSlot,
                bankSignatureCount: bankSignatureCount,
                parentBankHash: parentBankHash,
                bankHash: bankHash,
                blockhash: blockhash,
                accountsLtHash: accountsLtHash,
                transactionStatusRoot: transactionStatusRoot,
                accountInclusionRoot: accountInclusionRoot,
                accountsLtHashChecksum: accountsLtHashChecksum
            )
        )
        XCTAssertThrowsError(try solanaSccpBankForkHash(
            epoch: 4,
            finalizedSlot: finalizedSlot,
            parentSlot: parentSlot,
            bankSignatureCount: bankSignatureCount,
            parentBankHash: parentBankHash,
            bankHash: bankHash,
            blockhash: blockhash,
            accountsLtHash: accountsLtHash,
            transactionStatusRoot: transactionStatusRoot,
            accountInclusionRoot: accountInclusionRoot,
            accountsLtHashChecksum: accountsLtHashChecksum
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("epoch"))
        }
        XCTAssertThrowsError(try solanaSccpBankForkHash(
            finalizedSlot: finalizedSlot,
            parentSlot: parentSlot - 1,
            bankSignatureCount: bankSignatureCount,
            parentBankHash: parentBankHash,
            bankHash: bankHash,
            blockhash: blockhash,
            accountsLtHash: accountsLtHash,
            transactionStatusRoot: transactionStatusRoot,
            accountInclusionRoot: accountInclusionRoot,
            accountsLtHashChecksum: accountsLtHashChecksum
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("parentSlot"))
        }
        XCTAssertThrowsError(try solanaSccpBankForkHash(
            finalizedSlot: finalizedSlot,
            parentSlot: parentSlot,
            bankSignatureCount: 0,
            parentBankHash: parentBankHash,
            bankHash: bankHash,
            blockhash: blockhash,
            accountsLtHash: accountsLtHash,
            transactionStatusRoot: transactionStatusRoot,
            accountInclusionRoot: accountInclusionRoot,
            accountsLtHashChecksum: accountsLtHashChecksum
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("bankSignatureCount"))
        }
        XCTAssertThrowsError(try solanaSccpBankForkHash(
            finalizedSlot: finalizedSlot,
            parentSlot: parentSlot,
            bankSignatureCount: bankSignatureCount,
            parentBankHash: parentBankHash,
            bankHash: parentBankHash,
            blockhash: blockhash,
            accountsLtHash: accountsLtHash,
            transactionStatusRoot: transactionStatusRoot,
            accountInclusionRoot: accountInclusionRoot,
            accountsLtHashChecksum: accountsLtHashChecksum
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("bankHash"))
        }
        XCTAssertThrowsError(try solanaSccpBankForkHash(
            finalizedSlot: finalizedSlot,
            parentSlot: parentSlot,
            bankSignatureCount: bankSignatureCount,
            parentBankHash: parentBankHash,
            bankHash: "0x" + String(repeating: "44", count: 32),
            blockhash: blockhash,
            accountsLtHash: accountsLtHash,
            transactionStatusRoot: transactionStatusRoot,
            accountInclusionRoot: accountInclusionRoot,
            accountsLtHashChecksum: accountsLtHashChecksum
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("bankHash"))
        }
        XCTAssertThrowsError(try solanaSccpBankForkHash(
            finalizedSlot: finalizedSlot,
            parentSlot: parentSlot,
            bankSignatureCount: bankSignatureCount,
            parentBankHash: parentBankHash,
            bankHash: bankHash,
            blockhash: "0x" + String(repeating: "00", count: 32),
            accountsLtHash: accountsLtHash,
            transactionStatusRoot: transactionStatusRoot,
            accountInclusionRoot: accountInclusionRoot,
            accountsLtHashChecksum: accountsLtHashChecksum
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidHex32("blockhash"))
        }
        XCTAssertThrowsError(try solanaSccpBankForkHash(
            finalizedSlot: finalizedSlot,
            parentSlot: parentSlot,
            bankSignatureCount: bankSignatureCount,
            parentBankHash: parentBankHash,
            bankHash: bankHash,
            blockhash: blockhash,
            accountsLtHash: accountsLtHash,
            transactionStatusRoot: transactionStatusRoot,
            accountInclusionRoot: "0x" + String(repeating: "00", count: 32),
            accountsLtHashChecksum: accountsLtHashChecksum
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidHex32("accountInclusionRoot"))
        }
        XCTAssertThrowsError(try solanaSccpBankForkHash(
            finalizedSlot: finalizedSlot,
            parentSlot: parentSlot,
            bankSignatureCount: bankSignatureCount,
            parentBankHash: parentBankHash,
            bankHash: bankHash,
            blockhash: blockhash,
            accountsLtHash: accountsLtHash,
            transactionStatusRoot: transactionStatusRoot,
            accountInclusionRoot: accountInclusionRoot,
            accountsLtHashChecksum: "0x" + String(repeating: "00", count: 32)
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidHex32("accountsLtHashChecksum"))
        }
        XCTAssertThrowsError(try solanaSccpAgaveBankHash(
            parentBankHash: parentBankHash,
            bankSignatureCount: bankSignatureCount,
            blockhash: blockhash,
            accountsLtHash: accountsLtHash,
            bankHashHardForkData: Data(repeating: 0, count: 1_025)
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("bankHashHardForkData"))
        }
    }

    func testBuildsSolanaAccountInclusionWitness() throws {
        let finalizedSlot: UInt64 = 1_296_096
        let openings: [(address: Data, owner: Data, lamports: UInt64, dataHash: Data)] = [
            (Data(repeating: 0x31, count: 32), sccpSolanaVoteProgramId, 1_000_000, Data(repeating: 0x91, count: 32)),
            (Data(repeating: 0x41, count: 32), sccpSolanaStakeProgramId, 1_000_001, Data(repeating: 0x92, count: 32)),
            (Data(repeating: 0x51, count: 32), sccpSolanaStakeProgramId, 1_000_002, Data(repeating: 0x93, count: 32)),
        ]
        let openingInputs = openings.map { opening in
            SolanaSccpAccountOpeningInput(
                address: opening.address,
                owner: opening.owner,
                lamports: opening.lamports,
                rentEpoch: 0,
                dataHash: "0x" + opening.dataHash.hexEncodedString()
            )
        }
        let rawData = [
            Data(repeating: 0x01, count: 64),
            Data(repeating: 0x02, count: 64),
            Data(repeating: 0x03, count: 64),
        ]
        let rawDataHash0 = Data(hexString: String((try solanaSccpAccountRawDataHash(rawData[0])).dropFirst(2)))!

        XCTAssertEqual(
            try canonicalSolanaSccpAccountInclusionLeafBytes(
                finalizedSlot: finalizedSlot,
                address: openings[0].address,
                owner: openings[0].owner,
                lamports: openings[0].lamports,
                rentEpoch: 0,
                dataHash: openings[0].dataHash,
                rawDataHash: rawDataHash0
            ).count,
            109
        )
        let leaves = try openings.enumerated().map { index, opening in
            try solanaSccpAccountInclusionLeafHash(
                finalizedSlot: finalizedSlot,
                address: opening.address,
                owner: opening.owner,
                lamports: opening.lamports,
                rentEpoch: 0,
                dataHash: opening.dataHash,
                rawData: rawData[index]
            )
        }
        XCTAssertEqual(try canonicalSolanaSccpAccountInclusionNodeBytes(left: leaves[0], right: leaves[1]).count, 65)
        XCTAssertTrue(try solanaSccpAccountInclusionNodeHash(left: leaves[0], right: leaves[1]).hasPrefix("0x"))

        let witness = try solanaSccpAccountInclusionRootAndBranches(leaves: leaves)
        XCTAssertEqual(witness.branches.count, leaves.count)
        XCTAssertEqual(
            try solanaSccpAccountInclusionRootFromBranch(leaf: leaves[0], siblings: witness.branches[0]),
            witness.root
        )
        XCTAssertEqual(
            try solanaSccpAccountInclusionRootFromBranch(leaf: leaves[1], siblings: witness.branches[1]),
            witness.root
        )
        let openedWitness = try solanaSccpOpenedAccountInclusionWitness(
            SolanaSccpOpenedAccountInclusionWitnessInput(
                finalizedSlot: finalizedSlot,
                validatorVoteAccountOpenings: [openingInputs[0]],
                validatorVoteAccountRawData: [rawData[0]],
                validatorStakeAccountOpenings: [openingInputs[1]],
                validatorStakeAccountRawData: [rawData[1]],
                stakeHistorySysvarOpening: openingInputs[2],
                stakeHistorySysvarRawData: rawData[2],
                expectedAccountInclusionRoot: witness.root
            )
        )
        XCTAssertEqual(openedWitness.branches, witness.branches)
        XCTAssertEqual(openedWitness.validatorVoteAccountBranches, [witness.branches[0]])
        XCTAssertEqual(openedWitness.validatorStakeAccountBranches, [witness.branches[1]])
        XCTAssertEqual(openedWitness.stakeHistorySysvarBranch, witness.branches[2])
        let duplicateStakeOpening = SolanaSccpAccountOpeningInput(
            address: openingInputs[0].address,
            owner: openingInputs[1].owner,
            lamports: openingInputs[1].lamports,
            rentEpoch: openingInputs[1].rentEpoch,
            executable: openingInputs[1].executable,
            dataHash: openingInputs[1].dataHash
        )
        XCTAssertThrowsError(try solanaSccpOpenedAccountInclusionWitness(
            SolanaSccpOpenedAccountInclusionWitnessInput(
                finalizedSlot: finalizedSlot,
                validatorVoteAccountOpenings: [openingInputs[0]],
                validatorVoteAccountRawData: [rawData[0]],
                validatorStakeAccountOpenings: [duplicateStakeOpening],
                validatorStakeAccountRawData: [rawData[1]],
                stakeHistorySysvarOpening: openingInputs[2],
                stakeHistorySysvarRawData: rawData[2]
            )
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("openedAccountAddresses"))
        }
        XCTAssertThrowsError(try solanaSccpOpenedAccountInclusionWitness(
            SolanaSccpOpenedAccountInclusionWitnessInput(
                finalizedSlot: finalizedSlot,
                validatorVoteAccountOpenings: [openingInputs[0]],
                validatorVoteAccountRawData: [rawData[0]],
                validatorStakeAccountOpenings: [openingInputs[1]],
                validatorStakeAccountRawData: [rawData[1]],
                stakeHistorySysvarOpening: openingInputs[2],
                stakeHistorySysvarRawData: rawData[2],
                expectedAccountInclusionRoot: "0x" + String(repeating: "77", count: 32)
            )
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("accountInclusionRoot"))
        }
        let mutatedLeaf = try solanaSccpAccountInclusionLeafHash(
            finalizedSlot: finalizedSlot,
            address: openings[0].address,
            owner: openings[0].owner,
            lamports: openings[0].lamports,
            rentEpoch: 0,
            dataHash: openings[0].dataHash,
            rawData: Data(repeating: 0x04, count: 64)
        )
        XCTAssertNotEqual(
            try solanaSccpAccountInclusionRootFromBranch(leaf: mutatedLeaf, siblings: witness.branches[0]),
            witness.root
        )
        XCTAssertThrowsError(try solanaSccpAccountInclusionRootFromBranch(
            leaf: "0x" + String(repeating: "00", count: 32),
            siblings: []
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidHex32("leaf"))
        }
        XCTAssertThrowsError(try solanaSccpAccountInclusionRootFromBranch(
            leaf: leaves[0],
            siblings: Array(
                repeating: "0x" + String(repeating: "56", count: 32),
                count: sccpMaxSourceMerkleBranchNodes + 1
            )
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("siblings"))
        }
        XCTAssertThrowsError(try solanaSccpOpenedAccountInclusionWitness(
            SolanaSccpOpenedAccountInclusionWitnessInput(
                finalizedSlot: finalizedSlot,
                validatorVoteAccountOpenings: Array(
                    repeating: openingInputs[0],
                    count: sccpSolanaMaxValidators + 1
                ),
                validatorVoteAccountRawData: Array(
                    repeating: rawData[0],
                    count: sccpSolanaMaxValidators + 1
                ),
                validatorStakeAccountOpenings: [openingInputs[1]],
                validatorStakeAccountRawData: [rawData[1]],
                stakeHistorySysvarOpening: openingInputs[2],
                stakeHistorySysvarRawData: rawData[2]
            )
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("validatorVoteAccountOpenings"))
        }
        XCTAssertThrowsError(try solanaSccpAccountRawDataHash(Data()))
        XCTAssertThrowsError(try solanaSccpAccountInclusionRootAndBranches(leaves: [leaves[0], leaves[0]]))
    }

    func testDerivesAndValidatesMessageProofHashFromWitnessBranch() throws {
        let branch = [Data(repeating: 0x56, count: 32)]
        var witness = Self.sampleWitness(messageProofHash: "", inclusionBranch: branch)
        let derived = try solanaSccpMessageProofHash(
            sourceEventDigest: witness.sourceEventDigest,
            transactionStatusRoot: witness.transactionStatusRoot,
            transactionSignature: witness.transactionSignature,
            emitterProgramId: witness.emitterProgramId,
            inclusionBranch: branch
        )
        let normalized = try normalizeSolanaSccpWitness(witness)

        XCTAssertEqual(normalized.messageProofHash, derived)
        XCTAssertEqual(normalized.inclusionBranch, branch)
        XCTAssertThrowsError(try solanaSccpMessageProofHash(
            sourceEventDigest: String(repeating: "00", count: 32),
            transactionStatusRoot: witness.transactionStatusRoot,
            transactionSignature: witness.transactionSignature,
            emitterProgramId: witness.emitterProgramId,
            inclusionBranch: branch
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidHex32("sourceEventDigest"))
        }
        XCTAssertThrowsError(try solanaSccpMessageProofHash(
            sourceEventDigest: witness.sourceEventDigest,
            transactionStatusRoot: String(repeating: "00", count: 32),
            transactionSignature: witness.transactionSignature,
            emitterProgramId: witness.emitterProgramId,
            inclusionBranch: branch
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidHex32("transactionStatusRoot"))
        }
        XCTAssertThrowsError(try normalizeSolanaSccpWitness(Self.sampleWitness(
            transactionSignature: Self.solanaZeroSignature
        ))) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("transactionSignature"))
        }
        XCTAssertThrowsError(try normalizeSolanaSccpWitness(Self.sampleWitness(
            emitterProgramId: Self.solanaZeroProgram
        ))) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("emitterProgramId"))
        }
        XCTAssertGreaterThan(
            try canonicalSolanaSccpWitnessBytes(witness).count,
            try canonicalSolanaSccpWitnessBytes(Self.sampleWitness()).count
        )

        witness = Self.sampleWitness(
            messageProofHash: String(repeating: "cc", count: 32),
            inclusionBranch: branch
        )
        XCTAssertThrowsError(try normalizeSolanaSccpWitness(witness)) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .messageProofHashMismatch)
        }
    }

    func testRejectsUnexpectedSolanaSourceStateVerifierProfile() {
        XCTAssertThrowsError(try normalizeSolanaSccpWitness(Self.sampleWitness(
            sourceStateVerifierId: "debug-solana-state-verifier",
            sourceStateVerifierHash: String(repeating: "ab", count: 32)
        ))) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("sourceStateVerifierId"))
        }
    }

    func testRejectsSolanaWitnessAccountsLtHashChecksumMismatch() throws {
        let accountsLtHash = Data(repeating: 0x99, count: 2_048)
        guard let accountsLtHashChecksum = try? solanaSccpAccountsLtHashChecksum(accountsLtHash) else {
            throw XCTSkip("BLAKE3 bridge unavailable")
        }
        let parentBankHash = String(repeating: "c0", count: 32)
        let blockhash = String(repeating: "55", count: 32)
        guard let bankHash = try? solanaSccpAgaveBankHash(
            parentBankHash: parentBankHash,
            bankSignatureCount: 8,
            blockhash: blockhash,
            accountsLtHash: accountsLtHash
        ) else {
            throw XCTSkip("BLAKE3 bridge unavailable")
        }
        let valid = SolanaSccpWitnessInput(
            finalizedSlot: 321,
            parentSlot: 320,
            bankSignatureCount: 8,
            parentBankHash: parentBankHash,
            blockhash: blockhash,
            bankHash: bankHash,
            transactionStatusRoot: String(repeating: "bb", count: 32),
            messageProofHash: String(repeating: "cc", count: 32),
            accountInclusionRoot: String(repeating: "77", count: 32),
            accountsLtHashChecksum: accountsLtHashChecksum,
            accountsLtHash: accountsLtHash,
            transactionSignature: Self.solanaSignature55,
            emitterProgramId: Self.solanaProgram42,
            messageId: String(repeating: "dd", count: 32),
            payloadHash: String(repeating: "ee", count: 32),
            commitmentRoot: String(repeating: "12", count: 32),
            sourceEventDigest: String(repeating: "34", count: 32),
            statementHash: String(repeating: "56", count: 32),
            destinationBindingHash: String(repeating: "78", count: 32)
        )
        XCTAssertNoThrow(try normalizeSolanaSccpWitness(valid))

        let mismatch = SolanaSccpWitnessInput(
            finalizedSlot: 321,
            parentSlot: 320,
            bankSignatureCount: 8,
            parentBankHash: parentBankHash,
            blockhash: blockhash,
            bankHash: bankHash,
            transactionStatusRoot: String(repeating: "bb", count: 32),
            messageProofHash: String(repeating: "cc", count: 32),
            accountInclusionRoot: String(repeating: "77", count: 32),
            accountsLtHashChecksum: String(repeating: "88", count: 32),
            accountsLtHash: accountsLtHash,
            transactionSignature: Self.solanaSignature55,
            emitterProgramId: Self.solanaProgram42,
            messageId: String(repeating: "dd", count: 32),
            payloadHash: String(repeating: "ee", count: 32),
            commitmentRoot: String(repeating: "12", count: 32),
            sourceEventDigest: String(repeating: "34", count: 32),
            statementHash: String(repeating: "56", count: 32),
            destinationBindingHash: String(repeating: "78", count: 32)
        )
        XCTAssertThrowsError(try normalizeSolanaSccpWitness(mismatch)) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("accountsLtHashChecksum"))
        }
    }

    func testProverRequiresLinkedProofEngine() async throws {
        let prover = SolanaSccpProver()

        do {
            _ = try await prover.prove(Self.sampleWitness())
            XCTFail("expected localProverUnavailable")
        } catch let error as SolanaSccpProverError {
            XCTAssertEqual(error, .localProverUnavailable)
        }
    }

    func testRequiresSolanaProofContext() {
        XCTAssertThrowsError(try normalizeSolanaSccpProofContext(
            statementHash: "",
            destinationBindingHash: String(repeating: "78", count: 32)
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidHex32("statementHash"))
        }
        XCTAssertThrowsError(try normalizeSolanaSccpProofContext(
            statementHash: sccpZeroHashV1,
            destinationBindingHash: String(repeating: "78", count: 32)
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidHex32("statementHash"))
        }
        XCTAssertThrowsError(try normalizeSolanaSccpProofContext(
            statementHash: String(repeating: "56", count: 32),
            destinationBindingHash: ""
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidHex32("destinationBindingHash"))
        }
        XCTAssertThrowsError(try normalizeSolanaSccpProofContext(
            statementHash: String(repeating: "56", count: 32),
            destinationBindingHash: sccpZeroHashV1
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidHex32("destinationBindingHash"))
        }
    }

    func testBindsSourceAdapterDeploymentContextForUiProvers() throws {
        let request = try buildSolanaSccpProofRequest(Self.sampleWitness(
            sourceAdapterDeploymentHash: String(repeating: "ab", count: 32),
            sourceAdapterDeploymentReceiptHash: String(repeating: "cd", count: 32)
        ))

        XCTAssertEqual(
            request.publicInputs.sourceAdapterDeploymentHash,
            "0x" + String(repeating: "ab", count: 32)
        )
        XCTAssertEqual(
            request.publicInputs.sourceAdapterDeploymentReceiptHash,
            "0x" + String(repeating: "cd", count: 32)
        )
        XCTAssertEqual(
            try canonicalSccpSourceAdapterDeploymentBindingBytes(request.sourceAdapterDeploymentBinding).count,
            73
        )
        XCTAssertEqual(
            request.sourceAdapterDeploymentBindingHash,
            try sccpSourceAdapterDeploymentBindingHash(request.sourceAdapterDeploymentBinding)
        )

        XCTAssertThrowsError(try normalizeSccpSourceAdapterDeploymentBinding(
            sourceAdapterDeploymentHash: String(repeating: "ab", count: 32),
            sourceAdapterDeploymentReceiptHash: sccpZeroHashV1
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .sourceAdapterDeploymentBindingMismatch)
        }
        XCTAssertThrowsError(try normalizeSccpSourceAdapterDeploymentBinding(
            sourceAdapterDeploymentHash: String(repeating: "ab", count: 32),
            sourceAdapterDeploymentReceiptHash: String(repeating: "ab", count: 32)
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .sourceAdapterDeploymentBindingMismatch)
        }
    }

    func testDerivesSourceAdapterVerifierVkHashesForUiTooling() throws {
        let vectors: [(UInt32, String)] = [
            (sccpDomainEthereum, "0x2140903293411cad0f0eb217d8beb18d3a188edf7bba455098589a2409445e46"),
            (sccpDomainBsc, "0x12536f25748a6520f10ebd42a7bcccd6ec181b9d53129795c8e186dc6e8b18cc"),
            (sccpDomainSolana, "0xe7bc29d06bf56184183c3fc59a0e934cd1d8e16751f1eda2efaaf88aa350b9d6"),
            (sccpDomainTon, "0xf03f70e8cb504e69b0611df224c2783d04d8f4ee93beae7a62e1cd0a49703bad"),
            (sccpDomainTron, "0x0e12ad03def9d75887d4d6437e63539cef97c54db4769881eeda757a88826364"),
        ]
        for (sourceDomain, expectedHash) in vectors {
            XCTAssertEqual(
                try sccpSourceAdapterVerifierVkHash(sourceDomain: sourceDomain),
                expectedHash
            )
        }
        XCTAssertThrowsError(
            try sccpSourceAdapterVerifierVkHash(
                sourceDomain: sccpDomainTon,
                targetDomain: sccpDomainTon
            )
        ) { error in
            XCTAssertEqual(
                error as? SccpSourceProofHashError,
                .unsupportedSourceAdapterDomain("targetDomain")
            )
        }
    }
    func testDerivesEvmAndTronDestinationBindingsForUiTooling() throws {
        let evmBinding = try sccpEvmDestinationBinding(
            targetDomain: sccpDomainEthereum,
            networkId: "0x" + String(repeating: "33", count: 32),
            verifierAddress: "0x" + String(repeating: "11", count: 20),
            bridgeAddress: "0x" + String(repeating: "22", count: 20),
            verifierCodeHash: "0x" + String(repeating: "bb", count: 32),
            verifierKeyHash: "0x" + String(repeating: "cc", count: 32)
        )
        XCTAssertEqual(evmBinding.key, [
            "evm",
            "0",
            "1",
            String(repeating: "33", count: 32),
            "0x" + String(repeating: "11", count: 20),
            "0x" + String(repeating: "22", count: 20),
            "0x" + String(repeating: "bb", count: 32),
            "0x" + String(repeating: "cc", count: 32)
        ].joined(separator: ":"))
        XCTAssertEqual(
            evmBinding.hash,
            "0x3ad95ac3e5bc2892f768aae40a3b7ba673d561858b7d1318fbb9f6eba83207bf"
        )
        XCTAssertEqual(
            try sccpEvmDestinationBindingHash(
                targetDomain: sccpDomainEthereum,
                networkId: "0x" + String(repeating: "33", count: 32),
                verifierAddress: "0x" + String(repeating: "11", count: 20),
                bridgeAddress: "0x" + String(repeating: "22", count: 20),
                verifierCodeHash: "0x" + String(repeating: "bb", count: 32),
                verifierKeyHash: "0x" + String(repeating: "cc", count: 32)
            ),
            evmBinding.hash
        )

        let tronAddress = "TJRabPrwbZy45sbavfcjinPJC18kjpRTv8"
        let tronBinding = try sccpTronDestinationBinding(
            networkId: "0x" + String(repeating: "33", count: 32),
            verifierAddress: tronAddress,
            verifierCodeHash: "0x" + String(repeating: "bb", count: 32),
            verifierKeyHash: "0x" + String(repeating: "cc", count: 32)
        )
        XCTAssertEqual(tronBinding.key, [
            "tron",
            "0",
            "5",
            String(repeating: "33", count: 32),
            tronAddress,
            "0x" + String(repeating: "bb", count: 32),
            "0x" + String(repeating: "cc", count: 32)
        ].joined(separator: ":"))
        XCTAssertEqual(
            tronBinding.hash,
            "0x17c953ad5b8c9a2b6f7102aca993fa7c427d018505cf4f58fac35ea454caba7f"
        )
        XCTAssertEqual(
            try sccpTronDestinationBindingHash(
                networkId: "0x" + String(repeating: "33", count: 32),
                verifierAddress: tronAddress,
                verifierCodeHash: "0x" + String(repeating: "bb", count: 32),
                verifierKeyHash: "0x" + String(repeating: "cc", count: 32)
            ),
            tronBinding.hash
        )

        XCTAssertThrowsError(
            try sccpEvmDestinationBinding(
                targetDomain: sccpDomainEthereum,
                networkId: "0x" + String(repeating: "33", count: 32),
                verifierAddress: "0x" + String(repeating: "11", count: 20),
                bridgeAddress: "0x" + String(repeating: "11", count: 20),
                verifierCodeHash: "0x" + String(repeating: "bb", count: 32),
                verifierKeyHash: "0x" + String(repeating: "cc", count: 32)
            )
        ) { error in
            XCTAssertEqual(error as? SccpSourceProofHashError, .invalidSourceMaterial("bridgeAddress"))
        }
        XCTAssertThrowsError(
            try sccpTronDestinationBinding(
                networkId: "0x" + String(repeating: "33", count: 32),
                verifierAddress: "TJRabPrwbZy45sbavfcjinPJC18kjpRTv9",
                verifierCodeHash: "0x" + String(repeating: "bb", count: 32),
                verifierKeyHash: "0x" + String(repeating: "cc", count: 32)
            )
        ) { error in
            XCTAssertEqual(error as? SccpSourceProofHashError, .invalidSourceMaterial("verifierAddress"))
        }
        XCTAssertThrowsError(
            try sccpTronDestinationBinding(
                networkId: "0x" + String(repeating: "33", count: 32),
                verifierAddress: " " + tronAddress,
                verifierCodeHash: "0x" + String(repeating: "bb", count: 32),
                verifierKeyHash: "0x" + String(repeating: "cc", count: 32)
            )
        ) { error in
            XCTAssertEqual(error as? SccpSourceProofHashError, .invalidSourceMaterial("verifierAddress"))
        }
    }

    func testDerivesSourceMaterialAndDeploymentRecordHashesForUiTooling() throws {
        let materialVectors: [(UInt32, String)] = [
            (sccpDomainEthereum, "0x4d1e9d15bc59c0a2157aa967eb033f5778c805aea4707785a31ef6b60f694d77"),
            (sccpDomainBsc, "0x1630e4d75e2676cc443e07b0477303240ae4cff13bdf9fe61725b4a9a4ee959a"),
            (sccpDomainSolana, "0x499a7363142d5fcfe3a79b11a29ae2ad897e853649e80e39a162b8942f908331"),
            (sccpDomainTon, "0x08b11177113ac2d9f612abdf767a017de560d805e965b3dc32e28c8748ea2ebc"),
            (sccpDomainTron, "0x68c20262e44676bd5f3c4ec428f063373147a1ca14c5885648a9c651b3bcd8d8"),
        ]
        let deploymentVectors: [UInt32: String] = [
            sccpDomainEthereum: "0xfeb62925410b1376a2cd3704c3822e335da96c3dcc283b041a559d7b08ab1cc4",
            sccpDomainBsc: "0x7d47ade779a5bddb3a5f283600af677db8605b75a00516a4328f3823ff28fb2d",
            sccpDomainSolana: "0xcdb2a81cb31e58d9bc1f4292d33c3f4990b2d2008dda1b9b1275aaac087461cc",
            sccpDomainTon: "0x5c4e226c1f4619311762a9c889f8e3b99ea6f020317c2e8a0c76a08d7a70f887",
            sccpDomainTron: "0x94dbe28a2fb16e043b83639b6dea8ec62f53679599ef1dd220fd13c71c7bdcb8",
        ]
        for (domain, expectedMaterialHash) in materialVectors {
            XCTAssertFalse(try Self.sampleSourceVerifierMaterialBytes(domain: domain).isEmpty)
            XCTAssertEqual(try Self.sampleSourceVerifierMaterialHash(domain: domain), expectedMaterialHash)
            XCTAssertEqual(
                try Self.sampleSourceAdapterDeploymentHash(domain: domain),
                deploymentVectors[domain]
            )
        }
        XCTAssertThrowsError(
            try canonicalSccpSourceVerifierMaterialBytes(
                sourceDomain: sccpDomainEthereum,
                sourceTrustAnchorHash: "0x" + String(repeating: "44", count: 32),
                consensusVerifierHash: "0x" + String(repeating: "55", count: 32),
                messageInclusionVerifierHash: "0x" + String(repeating: "66", count: 32),
                finalityPolicyHash: "0x" + String(repeating: "88", count: 32),
                sourceStateVerifierHash: "0x" + String(repeating: "77", count: 32),
                bridgeAddress: "0x" + String(repeating: "11", count: 20),
                sourceBridgeEmitterCodeHash: "0x" + String(repeating: "77", count: 32)
            )
        ) { error in
            XCTAssertEqual(error as? SccpSourceProofHashError, .invalidSourceMaterial("sourceStateVerifierHash"))
        }
        XCTAssertThrowsError(
            try canonicalSccpSourceVerifierMaterialBytes(
                sourceDomain: sccpDomainSolana,
                sourceTrustAnchorHash: "0x" + String(repeating: "44", count: 32),
                consensusVerifierHash: "0x" + String(repeating: "55", count: 32),
                messageInclusionVerifierHash: "0x" + String(repeating: "66", count: 32),
                finalityPolicyHash: "0x" + String(repeating: "88", count: 32),
                sourceStateVerifierHash: "0x" + String(repeating: "77", count: 32),
                bridgeAddress: "0x" + String(repeating: "11", count: 20)
            )
        ) { error in
            XCTAssertEqual(error as? SccpSourceProofHashError, .invalidSourceMaterial("sourceBridgeEmitterAddress"))
        }
        XCTAssertThrowsError(
            try canonicalSccpSourceVerifierMaterialBytes(
                sourceDomain: sccpDomainEthereum,
                sourceTrustAnchorHash: "0x" + String(repeating: "44", count: 32),
                consensusVerifierHash: "0x" + String(repeating: "55", count: 32),
                messageInclusionVerifierHash: "0x" + String(repeating: "66", count: 32),
                finalityPolicyHash: "0x" + String(repeating: "88", count: 32),
                bridgeAddress: "0x" + String(repeating: "11", count: 20),
                sourceBridgeEmitterCodeHash: "0x" + String(repeating: "77", count: 32),
                networkId: "0x" + String(repeating: "33", count: 32),
                configHash: "0x871a910500648c68576f7d8fb044de1c494ae24c74f435c87dd451e6ae169c6b"
            )
        ) { error in
            XCTAssertEqual(error as? SccpSourceProofHashError, .invalidSourceMaterial("sourceBridgeNetworkId"))
        }
        XCTAssertThrowsError(
            try canonicalSccpSourceVerifierMaterialBytes(
                sourceDomain: sccpDomainEthereum,
                sourceTrustAnchorHash: "0x" + String(repeating: "44", count: 32),
                consensusVerifierHash: "0x" + String(repeating: "55", count: 32),
                messageInclusionVerifierHash: "0x" + String(repeating: "66", count: 32),
                finalityPolicyHash: "0x" + String(repeating: "88", count: 32),
                bridgeAddress: "0x" + String(repeating: "11", count: 20),
                sourceBridgeEmitterCodeHash: "0x" + String(repeating: "77", count: 32),
                networkId: sccpEthereumMainnetNetworkId,
                ownerAddress: "0x" + String(repeating: "22", count: 20),
                configHash: "0x871a910500648c68576f7d8fb044de1c494ae24c74f435c87dd451e6ae169c6b"
            )
        ) { error in
            XCTAssertEqual(error as? SccpSourceProofHashError, .invalidSourceMaterial("sourceBridgeOwnerAddress"))
        }
        XCTAssertThrowsError(
            try canonicalSccpSourceVerifierMaterialBytes(
                sourceDomain: sccpDomainEthereum,
                sourceTrustAnchorHash: "0x" + String(repeating: "44", count: 32),
                consensusVerifierHash: "0x" + String(repeating: "55", count: 32),
                messageInclusionVerifierHash: "0x" + String(repeating: "66", count: 32),
                finalityPolicyHash: "0x" + String(repeating: "88", count: 32),
                bridgeAddress: "0x" + String(repeating: "11", count: 20),
                sourceBridgeEmitterCodeHash: "0x" + String(repeating: "77", count: 32),
                networkId: sccpEthereumMainnetNetworkId,
                configHash: "0x" + String(repeating: "99", count: 32)
            )
        ) { error in
            XCTAssertEqual(error as? SccpSourceProofHashError, .invalidSourceMaterial("sourceBridgeConfigHash"))
        }
        let tonTemplateComponentHashes = [
            "sourceTrustAnchorHash": "0xd83b3a3eb920ac8338533535cf0d6c69c69d507e84aef8ec2094564b8427c56c",
            "consensusVerifierHash": "0xb0225e16477ea3420f7d0de76b87b6e99a43ab97f445d8565a384d4b655bc473",
            "messageInclusionVerifierHash": "0x89254256421c15da8c92842c7d6f448ef6c1d5ca1e2a173754643425fcee6353",
            "sourceStateVerifierHash": "0x540205f876591604ccf39f72a051ac5e82647c9e48dbd48cb129d2543971a34f",
            "finalityPolicyHash": "0x50044ee6db0eb0cdef097e69406b6c30d3406d8f784e8ba34e9b923b38bd0c43",
        ]
        for (field, templateHash) in tonTemplateComponentHashes {
            XCTAssertThrowsError(
                try canonicalSccpSourceVerifierMaterialBytes(
                    sourceDomain: sccpDomainTon,
                    sourceTrustAnchorHash: field == "sourceTrustAnchorHash" ? templateHash : "0x" + String(repeating: "44", count: 32),
                    consensusVerifierHash: field == "consensusVerifierHash" ? templateHash : "0x" + String(repeating: "55", count: 32),
                    messageInclusionVerifierHash: field == "messageInclusionVerifierHash" ? templateHash : "0x" + String(repeating: "66", count: 32),
                    finalityPolicyHash: field == "finalityPolicyHash" ? templateHash : "0x" + String(repeating: "88", count: 32),
                    sourceStateVerifierHash: field == "sourceStateVerifierHash" ? templateHash : "0x" + String(repeating: "77", count: 32)
                )
            ) { error in
                XCTAssertEqual(error as? SccpSourceProofHashError, .invalidSourceMaterial(field))
            }
        }
        let tronTemplateComponentHashes = [
            "sourceTrustAnchorHash": "0x3550934cbdfe49449ec4aa383dcea7674541fedf66ab6159b1ed2f2c0be4755c",
            "consensusVerifierHash": "0x8a1de96a869b2f28f197a7835597f17cf77ff45f7cbb77da2f7c48e87df8c5ea",
            "messageInclusionVerifierHash": "0xf39db56474b288680ad9561389cca7a841bd1fd223719255324705e1038fcacc",
            "finalityPolicyHash": "0xad5a6a4f200e070400b5aaa1b7976c639e67571eb711eb6f69d01e3615423864",
        ]
        for (field, templateHash) in tronTemplateComponentHashes {
            XCTAssertThrowsError(
                try canonicalSccpSourceVerifierMaterialBytes(
                    sourceDomain: sccpDomainTron,
                    sourceTrustAnchorHash: field == "sourceTrustAnchorHash" ? templateHash : "0x" + String(repeating: "44", count: 32),
                    consensusVerifierHash: field == "consensusVerifierHash" ? templateHash : "0x" + String(repeating: "55", count: 32),
                    messageInclusionVerifierHash: field == "messageInclusionVerifierHash" ? templateHash : "0x" + String(repeating: "66", count: 32),
                    finalityPolicyHash: field == "finalityPolicyHash" ? templateHash : "0x" + String(repeating: "88", count: 32),
                    bridgeAddress: "0x" + String(repeating: "11", count: 20),
                    sourceBridgeEmitterCodeHash: "0x" + String(repeating: "77", count: 32),
                    networkId: "0x" + String(repeating: "33", count: 32),
                    ownerAddress: "0x" + String(repeating: "22", count: 20),
                    configHash: "0xe986dd67bfa2307b4e00cf46bde41a88003a55c5b7fea311fa106614b2252f9d"
                )
            ) { error in
                XCTAssertEqual(error as? SccpSourceProofHashError, .invalidSourceMaterial(field))
            }
        }
        let solanaTemplateComponentHashes = [
            "sourceTrustAnchorHash": "0x113bdb7601d84f2098daec386346a7123857d181b3ac5bd23df50fa9e1b2cbe3",
            "consensusVerifierHash": "0x97ea89019e6c79305d06dfc27640ee14a6b42ba6eaf86e1835ee9b433dba48ba",
            "messageInclusionVerifierHash": "0xb8358bfef1e428a6a7e9115687cb2b88d9c21dad4021bea3e11d43489eb3dcb0",
            "sourceStateVerifierHash": sccpSolanaTemplateSourceStateVerifierHashV1,
            "finalityPolicyHash": "0x9df7ea90cf1bbba036788b14804f63f4be1e908390be89524fd4486f74344f56",
        ]
        for (field, templateHash) in solanaTemplateComponentHashes {
            XCTAssertThrowsError(
                try canonicalSccpSourceVerifierMaterialBytes(
                    sourceDomain: sccpDomainSolana,
                    sourceTrustAnchorHash: field == "sourceTrustAnchorHash" ? templateHash : "0x" + String(repeating: "44", count: 32),
                    consensusVerifierHash: field == "consensusVerifierHash" ? templateHash : "0x" + String(repeating: "55", count: 32),
                    messageInclusionVerifierHash: field == "messageInclusionVerifierHash" ? templateHash : "0x" + String(repeating: "66", count: 32),
                    finalityPolicyHash: field == "finalityPolicyHash" ? templateHash : "0x" + String(repeating: "88", count: 32),
                    sourceStateVerifierHash: field == "sourceStateVerifierHash" ? templateHash : "0x" + String(repeating: "77", count: 32)
                )
            ) { error in
                XCTAssertEqual(error as? SccpSourceProofHashError, .invalidSourceMaterial(field))
            }
        }
        XCTAssertThrowsError(
            try canonicalSccpSourceVerifierMaterialBytes(
                sourceDomain: sccpDomainTron,
                sourceTrustAnchorHash: "0x" + String(repeating: "44", count: 32),
                consensusVerifierHash: "0x" + String(repeating: "55", count: 32),
                messageInclusionVerifierHash: "0x" + String(repeating: "66", count: 32),
                finalityPolicyHash: "0x" + String(repeating: "88", count: 32),
                bridgeAddress: "0x" + String(repeating: "11", count: 20),
                sourceBridgeEmitterCodeHash: "0x" + String(repeating: "77", count: 32),
                networkId: "0x" + String(repeating: "33", count: 32),
                ownerAddress: "0x" + String(repeating: "22", count: 20),
                configHash: "0x" + String(repeating: "99", count: 32)
            )
        ) { error in
            XCTAssertEqual(
                error as? SccpSourceProofHashError,
                .invalidSourceMaterial("sourceBridgeConfigHash")
            )
        }
        XCTAssertThrowsError(
            try canonicalSccpSourceVerifierMaterialBytes(
                sourceDomain: sccpDomainEthereum,
                sourceTrustAnchorHash: "0x" + String(repeating: "44", count: 32),
                consensusVerifierHash: "0x" + String(repeating: "44", count: 32),
                messageInclusionVerifierHash: "0x" + String(repeating: "66", count: 32),
                finalityPolicyHash: "0x" + String(repeating: "88", count: 32),
                bridgeAddress: "0x" + String(repeating: "11", count: 20),
                sourceBridgeEmitterCodeHash: "0x" + String(repeating: "77", count: 32),
                networkId: sccpEthereumMainnetNetworkId,
                configHash: "0x871a910500648c68576f7d8fb044de1c494ae24c74f435c87dd451e6ae169c6b"
            )
        ) { error in
            guard case let .invalidSourceMaterial(field) = error as? SccpSourceProofHashError else {
                return XCTFail("expected source material role-hash error")
            }
            XCTAssertTrue(field.contains("sourceVerifierMaterialRoleHash"))
        }
        let ethAdapterVerifierHash = try sccpSourceAdapterVerifierVkHash(sourceDomain: sccpDomainEthereum)
        XCTAssertThrowsError(
            try canonicalSccpSourceAdapterEngineDeploymentBytes(
                sourceDomain: sccpDomainEthereum,
                sourceTrustAnchorHash: "0x" + String(repeating: "44", count: 32),
                consensusVerifierHash: "0x" + String(repeating: "55", count: 32),
                messageInclusionVerifierHash: "0x" + String(repeating: "66", count: 32),
                finalityPolicyHash: "0x" + String(repeating: "88", count: 32),
                deploymentReceiptHash: ethAdapterVerifierHash,
                bridgeAddress: "0x" + String(repeating: "11", count: 20),
                sourceBridgeEmitterCodeHash: "0x" + String(repeating: "77", count: 32),
                networkId: sccpEthereumMainnetNetworkId,
                configHash: "0x871a910500648c68576f7d8fb044de1c494ae24c74f435c87dd451e6ae169c6b"
            )
        ) { error in
            guard case let .invalidSourceMaterial(field) = error as? SccpSourceProofHashError else {
                return XCTFail("expected source deployment role-hash error")
            }
            XCTAssertTrue(field.contains("sourceAdapterDeploymentRoleHash"))
        }
        XCTAssertEqual(
            try Self.sampleSourceAdapterDeploymentHash(
                domain: sccpDomainSolana,
                solanaTowerReplayVerifierHash: "0x" + String(repeating: "bb", count: 32),
                solanaFullAccountsdbLatticeVerifierHash: "0x" + String(repeating: "cc", count: 32),
                solanaBankForkChoiceVerifierHash: "0x" + String(repeating: "dd", count: 32)
            ),
            "0x97e5c4196aff6387b9d973e663de3ce9345e1d8c3de89d22505b2197e282dc61"
        )
        XCTAssertEqual(
            try Self.sampleSolanaFullLightClientGateHash(),
            "0x2c94b86a665bb68708b762c678661f5e9879bd588627e93a640796eeaef970f9"
        )
        XCTAssertThrowsError(try Self.sampleSolanaFullLightClientGateHash(towerReplayHash: "0x" + String(repeating: "00", count: 32))) { error in
            XCTAssertEqual(error as? SccpSourceProofHashError, .invalidHex32("solanaTowerReplayVerifierHash"))
        }
        XCTAssertThrowsError(
            try Self.sampleSolanaFullLightClientGateHash(
                towerReplayHash: "0x" + String(repeating: "bb", count: 32),
                fullAccountsdbLatticeHash: "0x" + String(repeating: "bb", count: 32)
            )
        ) { error in
            XCTAssertEqual(error as? SccpSourceProofHashError, .invalidSourceMaterial("solanaAuditVerifierHash"))
        }
        XCTAssertThrowsError(
            try Self.sampleSolanaFullLightClientGateHash(
                towerReplayHash: Self.sourceStateVerifierHash(domain: sccpDomainSolana)!
            )
        ) { error in
            XCTAssertEqual(error as? SccpSourceProofHashError, .invalidSourceMaterial("solanaAuditVerifierHash"))
        }
        XCTAssertThrowsError(
            try Self.sampleSolanaFullLightClientGateHash(
                towerReplayHash: "0x113bdb7601d84f2098daec386346a7123857d181b3ac5bd23df50fa9e1b2cbe3"
            )
        ) { error in
            XCTAssertEqual(error as? SccpSourceProofHashError, .invalidSourceMaterial("solanaAuditVerifierHash"))
        }
        XCTAssertThrowsError(
            try Self.sampleSolanaFullLightClientGateHash(
                sourceStateHash: sccpSolanaTemplateSourceStateVerifierHashV1
            )
        ) { error in
            XCTAssertEqual(error as? SccpSourceProofHashError, .invalidSourceMaterial("sourceStateVerifierHash"))
        }
        XCTAssertThrowsError(
            try Self.sampleSourceAdapterDeploymentHash(
                domain: sccpDomainSolana,
                solanaTowerReplayVerifierHash: "0x" + String(repeating: "bb", count: 32)
            )
        ) { error in
            XCTAssertEqual(error as? SccpSourceProofHashError, .invalidSourceMaterial("solanaAuditVerifierHash"))
        }
        XCTAssertThrowsError(
            try Self.sampleSourceAdapterDeploymentHash(
                domain: sccpDomainTon,
                solanaTowerReplayVerifierHash: "0x" + String(repeating: "bb", count: 32),
                solanaFullAccountsdbLatticeVerifierHash: "0x" + String(repeating: "cc", count: 32),
                solanaBankForkChoiceVerifierHash: "0x" + String(repeating: "dd", count: 32)
            )
        ) { error in
            XCTAssertEqual(error as? SccpSourceProofHashError, .invalidSourceMaterial("solanaAuditVerifierHash"))
        }
        XCTAssertEqual(
            try Self.sampleSourceAdapterDeploymentHash(
                domain: sccpDomainTon,
                tonMasterchainConfigVerifierHash: "0x" + String(repeating: "bb", count: 32),
                tonValidatorSetTransitionVerifierHash: "0x" + String(repeating: "cc", count: 32),
                tonShardAccountsDictionaryVerifierHash: "0x" + String(repeating: "dd", count: 32)
            ),
            "0x61e5d710ccbc902be00a38a5a80d05c19de97105605a3f93d4f8067862d81f07"
        )
        XCTAssertEqual(
            try Self.sampleTonFullLightClientGateHash(),
            "0xc32d8cfc2e273646abb00911b9a15e7ee0ab1721b04a6e89a060422dd3cc4596"
        )
        XCTAssertThrowsError(try Self.sampleTonFullLightClientGateHash(masterchainConfigHash: "0x" + String(repeating: "00", count: 32))) { error in
            XCTAssertEqual(error as? SccpSourceProofHashError, .invalidHex32("tonMasterchainConfigVerifierHash"))
        }
        XCTAssertThrowsError(try Self.sampleTonFullLightClientGateHash(
            masterchainConfigHash: "0x" + String(repeating: "bb", count: 32),
            validatorSetTransitionHash: "0x" + String(repeating: "bb", count: 32)
        )) { error in
            XCTAssertEqual(error as? SccpSourceProofHashError, .invalidSourceMaterial("tonAuditVerifierHash"))
        }
        XCTAssertThrowsError(try Self.sampleTonFullLightClientGateHash(
            masterchainConfigHash: Self.sourceStateVerifierHash(domain: sccpDomainTon)!
        )) { error in
            XCTAssertEqual(error as? SccpSourceProofHashError, .invalidSourceMaterial("tonAuditVerifierHash"))
        }
        XCTAssertThrowsError(try Self.sampleTonFullLightClientGateHash(
            masterchainConfigHash: "0xd83b3a3eb920ac8338533535cf0d6c69c69d507e84aef8ec2094564b8427c56c"
        )) { error in
            XCTAssertEqual(error as? SccpSourceProofHashError, .invalidSourceMaterial("tonAuditVerifierHash"))
        }
        XCTAssertThrowsError(
            try Self.sampleSourceAdapterDeploymentHash(
                domain: sccpDomainTon,
                tonMasterchainConfigVerifierHash: "0x" + String(repeating: "bb", count: 32)
            )
        ) { error in
            XCTAssertEqual(error as? SccpSourceProofHashError, .invalidSourceMaterial("tonAuditVerifierHash"))
        }
        XCTAssertThrowsError(
            try Self.sampleSourceAdapterDeploymentHash(
                domain: sccpDomainSolana,
                tonMasterchainConfigVerifierHash: "0x" + String(repeating: "bb", count: 32),
                tonValidatorSetTransitionVerifierHash: "0x" + String(repeating: "cc", count: 32),
                tonShardAccountsDictionaryVerifierHash: "0x" + String(repeating: "dd", count: 32)
            )
        ) { error in
            XCTAssertEqual(error as? SccpSourceProofHashError, .invalidSourceMaterial("tonAuditVerifierHash"))
        }
        XCTAssertThrowsError(
            try Self.sampleSourceAdapterDeploymentHash(
                domain: sccpDomainEthereum,
                adapterVerifierVkHash: "0x" + String(repeating: "99", count: 32)
            )
        ) { error in
            XCTAssertEqual(error as? SccpSourceProofHashError, .invalidSourceMaterial("adapterVerifierVkHash"))
        }
    }

    func testBuildsSolanaProgramInstructionSubmission() throws {
        let solanaDestinationBindingHash = try sccpDestinationBindingHash(domain: sccpDomainSolana)
        let request = try buildSolanaSccpProofRequest(Self.sampleProductionWitness(
            destinationBindingHash: solanaDestinationBindingHash
        ))
        let proofResult = try wrapSolanaSccpProofResult(
            proofBytes: Data([1, 2, 3, 4]),
            request: request
        )
        let submissionPublicInputs = Self.sampleSolanaSubmissionPublicInputs(from: request.publicInputs)

        let submission = try buildSolanaSccpSubmission(SolanaSccpSubmissionInput(
            publicInputs: submissionPublicInputs,
            proofResult: proofResult,
            bundleBytes: Data([5, 6, 7])
        ))

        XCTAssertEqual(submission.envelopeEncoding, sccpSolanaBorshInstructionV1)
        XCTAssertEqual(submission.submissionKind, "program_instruction")
        XCTAssertEqual(submission.verifierEntrypoint, "submit_sccp_message_proof")
        XCTAssertEqual(submission.arguments.map(\.key), [
            "proof_bytes",
            "public_inputs",
            "bundle_bytes",
            "statement_hash",
            "destination_binding_hash",
            "proof_context_hash",
        ])
        XCTAssertEqual(submission.publicInputsBytes.count, 141)
        XCTAssertEqual(
            submission.proofContextHash,
            try solanaSccpProofContextHash(normalizeSolanaSccpProofContext(
                statementHash: String(repeating: "56", count: 32),
                destinationBindingHash: solanaDestinationBindingHash
            ))
        )
        XCTAssertEqual(submission.instructionDataHex, submission.envelopeHex)
        XCTAssertEqual(
            String(data: Data(submission.instructionData.dropFirst(4).prefix(25)), encoding: .utf8),
            "submit_sccp_message_proof"
        )
        XCTAssertThrowsError(try SolanaSccpSubmissionInput(
            publicInputs: Self.sampleSolanaSubmissionPublicInputs(),
            proofResult: proofResult,
            bundleBytes: Data([5, 6, 7])
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("proofResult.publicInputs.bankHash"))
        }
        let uppercaseProofContext = SolanaSccpProofContext(
            version: 1,
            statementHash: proofResult.proofContext.statementHash.uppercased(),
            destinationBindingHash: proofResult.proofContext.destinationBindingHash.uppercased()
        )
        let uppercaseProofResult = SolanaSccpProofResult(
            version: proofResult.version,
            backend: proofResult.backend,
            proofBytes: proofResult.proofBytes,
            proofBase64: proofResult.proofBase64,
            publicInputs: SolanaSccpPublicInputs(
                messageId: proofResult.publicInputs.messageId,
                payloadHash: proofResult.publicInputs.payloadHash,
                commitmentRoot: proofResult.publicInputs.commitmentRoot,
                finalizedSlot: proofResult.publicInputs.finalizedSlot,
                parentSlot: proofResult.publicInputs.parentSlot,
                bankSignatureCount: proofResult.publicInputs.bankSignatureCount,
                parentBankHash: proofResult.publicInputs.parentBankHash,
                blockhash: proofResult.publicInputs.blockhash,
                bankHash: proofResult.publicInputs.bankHash,
                transactionStatusRoot: proofResult.publicInputs.transactionStatusRoot,
                messageProofHash: proofResult.publicInputs.messageProofHash,
                accountInclusionRoot: proofResult.publicInputs.accountInclusionRoot,
                accountsLtHashChecksum: proofResult.publicInputs.accountsLtHashChecksum,
                accountsLtHashProofPublicInputsHash: proofResult.publicInputs.accountsLtHashProofPublicInputsHash,
                sourceEventDigest: proofResult.publicInputs.sourceEventDigest,
                sourceStateVerifierId: proofResult.publicInputs.sourceStateVerifierId,
                sourceStateVerifierHash: proofResult.publicInputs.sourceStateVerifierHash,
                statementHash: proofResult.publicInputs.statementHash.uppercased(),
                destinationBindingHash: proofResult.publicInputs.destinationBindingHash.uppercased(),
                sourceAdapterDeploymentHash: proofResult.publicInputs.sourceAdapterDeploymentHash,
                sourceAdapterDeploymentReceiptHash: proofResult.publicInputs.sourceAdapterDeploymentReceiptHash,
                sourceAdapterDeploymentBindingHash: proofResult.publicInputs.sourceAdapterDeploymentBindingHash
            ),
            witnessHash: proofResult.witnessHash,
            proofContextHash: proofResult.proofContextHash,
            sourceAdapterDeploymentBindingHash: proofResult.sourceAdapterDeploymentBindingHash,
            sourceStateVerifierId: proofResult.sourceStateVerifierId,
            sourceStateVerifierHash: proofResult.sourceStateVerifierHash,
            proofContext: uppercaseProofContext,
            sourceAdapterDeploymentBinding: proofResult.sourceAdapterDeploymentBinding,
            envelopeHash: proofResult.envelopeHash
        )
        let normalizedMetadataSubmission = try buildSolanaSccpSubmission(SolanaSccpSubmissionInput(
            publicInputs: SolanaSccpSubmissionPublicInputs(
                messageId: submissionPublicInputs.messageId.uppercased(),
                payloadHash: submissionPublicInputs.payloadHash.uppercased(),
                targetDomain: submissionPublicInputs.targetDomain,
                commitmentRoot: submissionPublicInputs.commitmentRoot.uppercased(),
                finalityHeight: submissionPublicInputs.finalityHeight,
                finalityBlockHash: submissionPublicInputs.finalityBlockHash.uppercased()
            ),
            proofBytes: uppercaseProofResult.proofBytes,
            bundleBytes: Data([5, 6, 7]),
            statementHash: proofResult.proofContext.statementHash.uppercased(),
            destinationBindingHash: solanaDestinationBindingHash.uppercased(),
            proofContextHash: proofResult.proofContextHash.uppercased(),
            proofResult: uppercaseProofResult
        ))
        XCTAssertEqual(normalizedMetadataSubmission.proofContextHash, proofResult.proofContextHash)
        XCTAssertEqual(normalizedMetadataSubmission.statementHash, proofResult.proofContext.statementHash)
        XCTAssertEqual(normalizedMetadataSubmission.destinationBindingHash, solanaDestinationBindingHash)
        let missingEnvelopeProofResult = SolanaSccpProofResult(
            version: proofResult.version,
            backend: proofResult.backend,
            proofBytes: proofResult.proofBytes,
            proofBase64: proofResult.proofBase64,
            publicInputs: proofResult.publicInputs,
            witnessHash: proofResult.witnessHash,
            proofContextHash: proofResult.proofContextHash,
            sourceAdapterDeploymentBindingHash: proofResult.sourceAdapterDeploymentBindingHash,
            sourceStateVerifierId: proofResult.sourceStateVerifierId,
            sourceStateVerifierHash: proofResult.sourceStateVerifierHash,
            proofContext: proofResult.proofContext,
            sourceAdapterDeploymentBinding: proofResult.sourceAdapterDeploymentBinding,
            envelopeHash: sccpZeroHashV1
        )
        XCTAssertThrowsError(try SolanaSccpSubmissionInput(
            publicInputs: submissionPublicInputs,
            proofResult: missingEnvelopeProofResult,
            bundleBytes: Data([5, 6, 7])
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidHex32("proofResult.envelopeHash"))
        }
        let tamperedEnvelopeProofResult = SolanaSccpProofResult(
            version: proofResult.version,
            backend: proofResult.backend,
            proofBytes: proofResult.proofBytes,
            proofBase64: proofResult.proofBase64,
            publicInputs: proofResult.publicInputs,
            witnessHash: proofResult.witnessHash,
            proofContextHash: proofResult.proofContextHash,
            sourceAdapterDeploymentBindingHash: proofResult.sourceAdapterDeploymentBindingHash,
            sourceStateVerifierId: proofResult.sourceStateVerifierId,
            sourceStateVerifierHash: proofResult.sourceStateVerifierHash,
            proofContext: proofResult.proofContext,
            sourceAdapterDeploymentBinding: proofResult.sourceAdapterDeploymentBinding,
            envelopeHash: "0x" + String(repeating: "aa", count: 32)
        )
        XCTAssertThrowsError(try SolanaSccpSubmissionInput(
            publicInputs: submissionPublicInputs,
            proofResult: tamperedEnvelopeProofResult,
            bundleBytes: Data([5, 6, 7])
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("proofResult.envelopeHash"))
        }
        let tamperedDeploymentBindingProofResult = SolanaSccpProofResult(
            version: proofResult.version,
            backend: proofResult.backend,
            proofBytes: proofResult.proofBytes,
            proofBase64: proofResult.proofBase64,
            publicInputs: proofResult.publicInputs,
            witnessHash: proofResult.witnessHash,
            proofContextHash: proofResult.proofContextHash,
            sourceAdapterDeploymentBindingHash: proofResult.sourceAdapterDeploymentBindingHash,
            sourceStateVerifierId: proofResult.sourceStateVerifierId,
            sourceStateVerifierHash: proofResult.sourceStateVerifierHash,
            proofContext: proofResult.proofContext,
            sourceAdapterDeploymentBinding: SolanaSccpSourceAdapterDeploymentBinding(
                version: proofResult.sourceAdapterDeploymentBinding.version,
                sourceDomain: proofResult.sourceAdapterDeploymentBinding.sourceDomain,
                targetDomain: proofResult.sourceAdapterDeploymentBinding.targetDomain,
                sourceAdapterDeploymentHash: "0x" + String(repeating: "ee", count: 32),
                sourceAdapterDeploymentReceiptHash: proofResult.sourceAdapterDeploymentBinding.sourceAdapterDeploymentReceiptHash
            ),
            envelopeHash: proofResult.envelopeHash
        )
        XCTAssertThrowsError(try SolanaSccpSubmissionInput(
            publicInputs: submissionPublicInputs,
            proofResult: tamperedDeploymentBindingProofResult,
            bundleBytes: Data([5, 6, 7])
        )) { error in
            XCTAssertEqual(
                error as? SolanaSccpProverError,
                .invalidString("proofResult.sourceAdapterDeploymentBindingHash")
            )
        }
        func proofResultPublicInputs(
            parentSlot: UInt64? = nil,
            messageProofHash: String? = nil,
            sourceStateVerifierId: String? = nil,
            sourceStateVerifierHash: String? = nil,
            sourceAdapterDeploymentHash: String? = nil
        ) -> SolanaSccpPublicInputs {
            SolanaSccpPublicInputs(
                messageId: proofResult.publicInputs.messageId,
                payloadHash: proofResult.publicInputs.payloadHash,
                commitmentRoot: proofResult.publicInputs.commitmentRoot,
                finalizedSlot: proofResult.publicInputs.finalizedSlot,
                parentSlot: parentSlot ?? proofResult.publicInputs.parentSlot,
                bankSignatureCount: proofResult.publicInputs.bankSignatureCount,
                parentBankHash: proofResult.publicInputs.parentBankHash,
                blockhash: proofResult.publicInputs.blockhash,
                bankHash: proofResult.publicInputs.bankHash,
                transactionStatusRoot: proofResult.publicInputs.transactionStatusRoot,
                messageProofHash: messageProofHash ?? proofResult.publicInputs.messageProofHash,
                accountInclusionRoot: proofResult.publicInputs.accountInclusionRoot,
                accountsLtHashChecksum: proofResult.publicInputs.accountsLtHashChecksum,
                accountsLtHashProofPublicInputsHash: proofResult.publicInputs.accountsLtHashProofPublicInputsHash,
                sourceEventDigest: proofResult.publicInputs.sourceEventDigest,
                sourceStateVerifierId: sourceStateVerifierId ?? proofResult.publicInputs.sourceStateVerifierId,
                sourceStateVerifierHash: sourceStateVerifierHash ?? proofResult.publicInputs.sourceStateVerifierHash,
                statementHash: proofResult.publicInputs.statementHash,
                destinationBindingHash: proofResult.publicInputs.destinationBindingHash,
                sourceAdapterDeploymentHash: sourceAdapterDeploymentHash ?? proofResult.publicInputs.sourceAdapterDeploymentHash,
                sourceAdapterDeploymentReceiptHash: proofResult.publicInputs.sourceAdapterDeploymentReceiptHash,
                sourceAdapterDeploymentBindingHash: proofResult.publicInputs.sourceAdapterDeploymentBindingHash
            )
        }
        func makeProofResult(
            version: UInt8 = proofResult.version,
            proofBase64: String = proofResult.proofBase64,
            publicInputs: SolanaSccpPublicInputs = proofResult.publicInputs,
            witnessHash: String = proofResult.witnessHash,
            proofContext: SolanaSccpProofContext = proofResult.proofContext,
            sourceAdapterDeploymentBinding: SolanaSccpSourceAdapterDeploymentBinding = proofResult.sourceAdapterDeploymentBinding
        ) -> SolanaSccpProofResult {
            SolanaSccpProofResult(
                version: version,
                backend: proofResult.backend,
                proofBytes: proofResult.proofBytes,
                proofBase64: proofBase64,
                publicInputs: publicInputs,
                witnessHash: witnessHash,
                proofContextHash: proofResult.proofContextHash,
                sourceAdapterDeploymentBindingHash: proofResult.sourceAdapterDeploymentBindingHash,
                sourceStateVerifierId: proofResult.sourceStateVerifierId,
                sourceStateVerifierHash: proofResult.sourceStateVerifierHash,
                proofContext: proofContext,
                sourceAdapterDeploymentBinding: sourceAdapterDeploymentBinding,
                envelopeHash: proofResult.envelopeHash
            )
        }
        XCTAssertThrowsError(try SolanaSccpSubmissionInput(
            publicInputs: submissionPublicInputs,
            proofResult: makeProofResult(version: 2),
            bundleBytes: Data([5, 6, 7])
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("proofResult.version"))
        }
        XCTAssertThrowsError(try SolanaSccpSubmissionInput(
            publicInputs: submissionPublicInputs,
            proofResult: makeProofResult(proofBase64: "AAAA"),
            bundleBytes: Data([5, 6, 7])
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("proofResult.proofBase64"))
        }
        XCTAssertThrowsError(try SolanaSccpSubmissionInput(
            publicInputs: submissionPublicInputs,
            proofResult: makeProofResult(witnessHash: sccpZeroHashV1),
            bundleBytes: Data([5, 6, 7])
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidHex32("proofResult.witnessHash"))
        }
        XCTAssertThrowsError(try SolanaSccpSubmissionInput(
            publicInputs: submissionPublicInputs,
            proofResult: makeProofResult(proofContext: SolanaSccpProofContext(
                version: 2,
                statementHash: proofResult.proofContext.statementHash,
                destinationBindingHash: proofResult.proofContext.destinationBindingHash
            )),
            bundleBytes: Data([5, 6, 7])
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("proofResult.proofContext.version"))
        }
        XCTAssertThrowsError(try SolanaSccpSubmissionInput(
            publicInputs: submissionPublicInputs,
            proofResult: makeProofResult(sourceAdapterDeploymentBinding: SolanaSccpSourceAdapterDeploymentBinding(
                version: 2,
                sourceDomain: proofResult.sourceAdapterDeploymentBinding.sourceDomain,
                targetDomain: proofResult.sourceAdapterDeploymentBinding.targetDomain,
                sourceAdapterDeploymentHash: proofResult.sourceAdapterDeploymentBinding.sourceAdapterDeploymentHash,
                sourceAdapterDeploymentReceiptHash: proofResult.sourceAdapterDeploymentBinding.sourceAdapterDeploymentReceiptHash
            )),
            bundleBytes: Data([5, 6, 7])
        )) { error in
            XCTAssertEqual(
                error as? SolanaSccpProverError,
                .invalidString("proofResult.sourceAdapterDeploymentBinding.version")
            )
        }
        XCTAssertThrowsError(try SolanaSccpSubmissionInput(
            publicInputs: submissionPublicInputs,
            proofResult: makeProofResult(publicInputs: proofResultPublicInputs(
                parentSlot: proofResult.publicInputs.finalizedSlot
            )),
            bundleBytes: Data([5, 6, 7])
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("proofResult.publicInputs.parentSlot"))
        }
        XCTAssertThrowsError(try SolanaSccpSubmissionInput(
            publicInputs: submissionPublicInputs,
            proofResult: makeProofResult(publicInputs: proofResultPublicInputs(messageProofHash: sccpZeroHashV1)),
            bundleBytes: Data([5, 6, 7])
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidHex32("proofResult.publicInputs.messageProofHash"))
        }
        let tamperedDeploymentPublicInputs = proofResultPublicInputs(
            sourceAdapterDeploymentHash: "0x" + String(repeating: "ee", count: 32)
        )
        let tamperedDeploymentPublicInputsProofResult = SolanaSccpProofResult(
            version: proofResult.version,
            backend: proofResult.backend,
            proofBytes: proofResult.proofBytes,
            proofBase64: proofResult.proofBase64,
            publicInputs: tamperedDeploymentPublicInputs,
            witnessHash: proofResult.witnessHash,
            proofContextHash: proofResult.proofContextHash,
            sourceAdapterDeploymentBindingHash: proofResult.sourceAdapterDeploymentBindingHash,
            sourceStateVerifierId: proofResult.sourceStateVerifierId,
            sourceStateVerifierHash: proofResult.sourceStateVerifierHash,
            proofContext: proofResult.proofContext,
            sourceAdapterDeploymentBinding: proofResult.sourceAdapterDeploymentBinding,
            envelopeHash: proofResult.envelopeHash
        )
        XCTAssertThrowsError(try SolanaSccpSubmissionInput(
            publicInputs: submissionPublicInputs,
            proofResult: tamperedDeploymentPublicInputsProofResult,
            bundleBytes: Data([5, 6, 7])
        )) { error in
            XCTAssertEqual(
                error as? SolanaSccpProverError,
                .invalidString("proofResult.publicInputs.sourceAdapterDeploymentHash")
            )
        }
        let tamperedSourceVerifierIdProofResult = SolanaSccpProofResult(
            version: proofResult.version,
            backend: proofResult.backend,
            proofBytes: proofResult.proofBytes,
            proofBase64: proofResult.proofBase64,
            publicInputs: proofResultPublicInputs(sourceStateVerifierId: "sccp:solana:wrong-source-state-verifier:v1"),
            witnessHash: proofResult.witnessHash,
            proofContextHash: proofResult.proofContextHash,
            sourceAdapterDeploymentBindingHash: proofResult.sourceAdapterDeploymentBindingHash,
            sourceStateVerifierId: proofResult.sourceStateVerifierId,
            sourceStateVerifierHash: proofResult.sourceStateVerifierHash,
            proofContext: proofResult.proofContext,
            sourceAdapterDeploymentBinding: proofResult.sourceAdapterDeploymentBinding,
            envelopeHash: proofResult.envelopeHash
        )
        XCTAssertThrowsError(try SolanaSccpSubmissionInput(
            publicInputs: submissionPublicInputs,
            proofResult: tamperedSourceVerifierIdProofResult,
            bundleBytes: Data([5, 6, 7])
        )) { error in
            XCTAssertEqual(
                error as? SolanaSccpProverError,
                .invalidString("proofResult.publicInputs.sourceStateVerifierId")
            )
        }
        let tamperedSourceVerifierHashProofResult = SolanaSccpProofResult(
            version: proofResult.version,
            backend: proofResult.backend,
            proofBytes: proofResult.proofBytes,
            proofBase64: proofResult.proofBase64,
            publicInputs: proofResultPublicInputs(sourceStateVerifierHash: "0x" + String(repeating: "dd", count: 32)),
            witnessHash: proofResult.witnessHash,
            proofContextHash: proofResult.proofContextHash,
            sourceAdapterDeploymentBindingHash: proofResult.sourceAdapterDeploymentBindingHash,
            sourceStateVerifierId: proofResult.sourceStateVerifierId,
            sourceStateVerifierHash: proofResult.sourceStateVerifierHash,
            proofContext: proofResult.proofContext,
            sourceAdapterDeploymentBinding: proofResult.sourceAdapterDeploymentBinding,
            envelopeHash: proofResult.envelopeHash
        )
        XCTAssertThrowsError(try SolanaSccpSubmissionInput(
            publicInputs: submissionPublicInputs,
            proofResult: tamperedSourceVerifierHashProofResult,
            bundleBytes: Data([5, 6, 7])
        )) { error in
            XCTAssertEqual(
                error as? SolanaSccpProverError,
                .invalidString("proofResult.publicInputs.sourceStateVerifierHash")
            )
        }
        XCTAssertThrowsError(try buildSolanaSccpSubmission(SolanaSccpSubmissionInput(
            publicInputs: Self.sampleSolanaSubmissionPublicInputs(version: 2),
            proofBytes: Data([1]),
            bundleBytes: Data([2]),
            statementHash: String(repeating: "56", count: 32),
            destinationBindingHash: solanaDestinationBindingHash
        ))) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("publicInputs.version"))
        }
        XCTAssertThrowsError(try buildSolanaSccpSubmission(SolanaSccpSubmissionInput(
            publicInputs: submissionPublicInputs,
            proofBytes: proofResult.proofBytes,
            bundleBytes: Data([2]),
            statementHash: proofResult.proofContext.statementHash,
            destinationBindingHash: proofResult.proofContext.destinationBindingHash
        ))) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("proofResult"))
        }
        XCTAssertThrowsError(try buildSolanaSccpSubmission(SolanaSccpSubmissionInput(
            publicInputs: Self.sampleSolanaSubmissionPublicInputs(targetDomain: sccpDomainSora),
            proofBytes: Data([1, 2]),
            bundleBytes: Data([5, 6, 7]),
            statementHash: String(repeating: "56", count: 32),
            destinationBindingHash: solanaDestinationBindingHash
        ))) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("publicInputs.targetDomain"))
        }
        XCTAssertThrowsError(try buildSolanaSccpSubmission(SolanaSccpSubmissionInput(
            publicInputs: submissionPublicInputs,
            proofBytes: proofResult.proofBytes,
            bundleBytes: Data([2]),
            statementHash: proofResult.proofContext.statementHash,
            destinationBindingHash: String(repeating: "78", count: 32),
            proofContextHash: proofResult.proofContextHash,
            proofResult: proofResult
        ))) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("destinationBindingHash"))
        }
        XCTAssertThrowsError(try buildSolanaSccpSubmission(SolanaSccpSubmissionInput(
            publicInputs: submissionPublicInputs,
            proofBytes: proofResult.proofBytes,
            bundleBytes: Data([0, 0]),
            statementHash: proofResult.proofContext.statementHash,
            destinationBindingHash: solanaDestinationBindingHash,
            proofContextHash: proofResult.proofContextHash,
            proofResult: proofResult
        ))) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("bundleBytes"))
        }
        XCTAssertThrowsError(try buildSolanaSccpSubmission(SolanaSccpSubmissionInput(
            publicInputs: submissionPublicInputs,
            proofBytes: proofResult.proofBytes,
            bundleBytes: Data(repeating: 1, count: sccpNativeRecursiveMaxProofBytes + 1),
            statementHash: proofResult.proofContext.statementHash,
            destinationBindingHash: solanaDestinationBindingHash,
            proofContextHash: proofResult.proofContextHash,
            proofResult: proofResult
        ))) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("bundleBytes"))
        }
        XCTAssertThrowsError(try buildSolanaSccpSubmission(SolanaSccpSubmissionInput(
            publicInputs: submissionPublicInputs,
            proofBytes: proofResult.proofBytes,
            bundleBytes: Data([2]),
            statementHash: proofResult.proofContext.statementHash,
            destinationBindingHash: solanaDestinationBindingHash,
            proofContextHash: String(repeating: "cc", count: 32),
            proofResult: proofResult
        ))) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .proofContextHashMismatch)
        }
    }

    func testProverWrapsExternalProofBytes() async throws {
        let productionWitness = Self.sampleProductionWitness()
        let prover = SolanaSccpProver { request in
            XCTAssertEqual(request.backend, sccpSolanaRecursiveProofBackendV1)
            XCTAssertEqual(request.proofContext.statementHash, "0x" + String(repeating: "56", count: 32))
            return Data([1, 2, 3, 4])
        }

        let result = try await prover.prove(productionWitness)

        XCTAssertEqual(result.proofBytes, Data([1, 2, 3, 4]))
        XCTAssertEqual(result.proofBase64, "AQIDBA==")
        XCTAssertEqual(result.proofContextHash, try buildSolanaSccpProofRequest(productionWitness).proofContextHash)
        XCTAssertTrue(result.envelopeHash.hasPrefix("0x"))
        XCTAssertEqual(result.envelopeHash.count, 66)

        let request = try buildSolanaSccpProofRequest(productionWitness)
        let directResult = try wrapSolanaSccpProofResult(
            proofBytes: Data([1, 2, 3, 4]),
            request: request
        )
        XCTAssertEqual(directResult, result)
        XCTAssertThrowsError(try wrapSolanaSccpProofResult(
            proofBytes: Data([0, 0]),
            request: request
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .allZeroProof)
        }
        XCTAssertThrowsError(try wrapSolanaSccpProofResult(
            proofBytes: Data(repeating: 1, count: sccpNativeRecursiveMaxProofBytes + 1),
            request: request
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("proofBytes"))
        }
        let mismatchedRequest = SolanaSccpProofRequest(
            version: request.version,
            backend: "debug-solana-backend",
            sourceDomain: request.sourceDomain,
            targetDomain: request.targetDomain,
            mainnetGenesisHash: request.mainnetGenesisHash,
            witnessHash: request.witnessHash,
            proofContextHash: request.proofContextHash,
            sourceAdapterDeploymentBindingHash: request.sourceAdapterDeploymentBindingHash,
            sourceStateVerifierId: request.sourceStateVerifierId,
            sourceStateVerifierHash: request.sourceStateVerifierHash,
            publicInputs: request.publicInputs,
            witness: request.witness,
            proofContext: request.proofContext,
            sourceAdapterDeploymentBinding: request.sourceAdapterDeploymentBinding
        )
        XCTAssertThrowsError(try wrapSolanaSccpProofResult(
            proofBytes: Data([1, 2, 3, 4]),
            request: mismatchedRequest
        )) { error in
            XCTAssertEqual(error as? SolanaSccpProverError, .invalidString("request"))
        }

        let missingProductionBinding = SolanaSccpProver { _ in
            XCTFail("local prover should not be invoked")
            return Data([1])
        }
        do {
            _ = try await missingProductionBinding.prove(Self.sampleProductionWitness(
                mainnetGenesisHash: "devnet"
            ))
            XCTFail("expected invalid mainnetGenesisHash")
        } catch let error as SolanaSccpProverError {
            XCTAssertEqual(error, .invalidString("mainnetGenesisHash"))
        }
        do {
            _ = try await missingProductionBinding.prove(Self.sampleProductionWitness(
                accountsLtHash: nil
            ))
            XCTFail("expected invalid accountsLtHash")
        } catch let error as SolanaSccpProverError {
            XCTAssertEqual(error, .invalidString("accountsLtHash"))
        }
        do {
            _ = try await missingProductionBinding.prove(Self.sampleWitness())
            XCTFail("expected invalid sourceStateVerifierHash")
        } catch let error as SolanaSccpProverError {
            XCTAssertEqual(error, .invalidHex32("sourceStateVerifierHash"))
        }
        do {
            _ = try await missingProductionBinding.prove(Self.sampleProductionWitness(
                sourceStateVerifierHash: sccpSolanaTemplateSourceStateVerifierHashV1
            ))
            XCTFail("expected invalid sourceStateVerifierHash")
        } catch let error as SolanaSccpProverError {
            XCTAssertEqual(error, .invalidHex32("sourceStateVerifierHash"))
        }
        do {
            _ = try await missingProductionBinding.prove(Self.sampleWitness(
                sourceStateVerifierHash: String(repeating: "ef", count: 32),
                sourceAdapterDeploymentHash: String(repeating: "ab", count: 32),
                sourceAdapterDeploymentReceiptHash: String(repeating: "cd", count: 32)
            ))
            XCTFail("expected invalid inclusionBranch")
        } catch let error as SolanaSccpProverError {
            XCTAssertEqual(error, .invalidString("inclusionBranch"))
        }
    }

    func testSolanaProverResolvesWitnessProviderBeforeBuildingRequest() async throws {
        let input = Self.sampleProductionWitness()
        let resolvedDestinationBindingHash = try sccpDestinationBindingHash(domain: sccpDomainSolana)
        let expectedRequest = try buildSolanaSccpProofRequest(
            Self.sampleProductionWitness(destinationBindingHash: resolvedDestinationBindingHash)
        )
        let witnessProvider = SolanaDestinationBindingWitnessProvider(
            resolvedDestinationBindingHash: resolvedDestinationBindingHash
        )
        let prover = SolanaSccpProver(
            witnessProvider: witnessProvider,
            proveFunction: { request in
                XCTAssertEqual(witnessProvider.resolveCount, 1)
                XCTAssertEqual(request.proofContext.destinationBindingHash, resolvedDestinationBindingHash)
                XCTAssertEqual(request.proofContextHash, expectedRequest.proofContextHash)
                return Data([1, 2, 3, 4])
            }
        )

        let result = try await prover.prove(input)

        XCTAssertEqual(witnessProvider.resolveCount, 1)
        XCTAssertEqual(result.witnessHash, expectedRequest.witnessHash)
        XCTAssertEqual(result.proofContextHash, expectedRequest.proofContextHash)
    }

    func testBuildsTonMessageBodyBoc() throws {
        let body = try buildTonSccpMessageBodyBoc(Self.sampleTonMessageBodyInput())

        XCTAssertEqual(Array(body.prefix(4)), [0xb5, 0xee, 0x9c, 0x72])
        XCTAssertGreaterThan(body.count, try canonicalTonSccpPublicInputsBytes(Self.sampleTonPublicInputs()).count)

        let destinationBinding = TonSccpSubmissionDestinationBindingInput(
            key: "sora:ton",
            bindingHash: String(repeating: "78", count: 32)
        )
        let manifest = TonSccpSubmissionManifestInput(
            messageBackend: "sccp-message-v1",
            registryBackend: "sccp-registry-v1",
            manifestSeed: "iroha:sccp:bridge-proof:message:stark-fri:v1:ton",
            destinationBinding: destinationBinding
        )
        let metadata = try canonicalTonSccpSubmissionMetadataBytes(TonSccpSubmissionMetadataInput(
            manifest: manifest,
            destinationBindingHash: String(repeating: "78", count: 32),
            publicInputs: Self.sampleTonPublicInputs(),
            statementHash: String(repeating: "bb", count: 32)
        ))
        XCTAssertGreaterThan(metadata.count, try canonicalTonSccpPublicInputsBytes(Self.sampleTonPublicInputs()).count)
        XCTAssertThrowsError(try canonicalTonSccpSubmissionMetadataBytes(TonSccpSubmissionMetadataInput(
            manifest: manifest,
            destinationBindingHash: String(repeating: "56", count: 32),
            publicInputs: Self.sampleTonPublicInputs(),
            statementHash: String(repeating: "bb", count: 32)
        ))) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("destinationBindingHash"))
        }
        XCTAssertThrowsError(try canonicalTonSccpSubmissionMetadataBytes(TonSccpSubmissionMetadataInput(
            manifest: TonSccpSubmissionManifestInput(
                counterpartyDomain: sccpDomainSolana,
                messageBackend: "sccp-message-v1",
                registryBackend: "sccp-registry-v1",
                manifestSeed: "iroha:sccp:bridge-proof:message:stark-fri:v1:ton",
                destinationBinding: destinationBinding
            ),
            publicInputs: Self.sampleTonPublicInputs(),
            statementHash: String(repeating: "bb", count: 32)
        ))) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("manifest.counterpartyDomain"))
        }

        let submission = try buildTonSccpSubmission(Self.sampleTonMessageBodyInput())
        XCTAssertEqual(submission.version, 1)
        XCTAssertEqual(submission.envelopeEncoding, sccpTonMessageBodyBocV1)
        XCTAssertEqual(submission.submissionKind, "internal_message")
        XCTAssertEqual(submission.verifierEntrypoint, "op::submit_sccp_message_proof")
        XCTAssertEqual(submission.messageBodyBoc, body)
        XCTAssertTrue(submission.messageBodyBocHex.hasPrefix("0xb5ee9c72"))
        XCTAssertEqual(submission.arguments.map(\.key), ["message_body_boc"])
        XCTAssertEqual(submission.arguments.map(\.encoding), ["ton_boc"])
        XCTAssertEqual(submission.arguments.map(\.bytesHex), [submission.messageBodyBocHex])
        XCTAssertEqual(submission.envelopeBytes, body)
        XCTAssertEqual(submission.envelopeHex, submission.messageBodyBocHex)
        XCTAssertThrowsError(try buildTonSccpSubmission(Self.sampleTonMessageBodyInput(bundleBytes: Data()))) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("bundleBytes"))
        }
        XCTAssertThrowsError(try buildTonSccpSubmission(Self.sampleTonMessageBodyInput(bundleBytes: Data([0, 0])))) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("bundleBytes"))
        }
        XCTAssertThrowsError(try buildTonSccpSubmission(Self.sampleTonMessageBodyInput(
            bundleBytes: Data(repeating: 1, count: sccpNativeRecursiveMaxProofBytes + 1)
        ))) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("bundleBytes"))
        }
        XCTAssertThrowsError(try buildTonSccpSubmission(Self.sampleTonMessageBodyInput(proofBytes: Data([0, 0])))) { error in
            XCTAssertEqual(error as? TonSccpProverError, .allZeroProof)
        }
        XCTAssertThrowsError(try buildTonSccpSubmission(Self.sampleTonMessageBodyInput(
            statementHash: String(repeating: "00", count: 32)
        ))) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidHex32("statementHash"))
        }
        XCTAssertThrowsError(try buildTonSccpSubmission(Self.sampleTonMessageBodyInput(
            destinationBindingHash: String(repeating: "00", count: 32)
        ))) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidHex32("destinationBindingHash"))
        }
        XCTAssertThrowsError(try buildTonSccpSubmission(Self.sampleTonMessageBodyInput(
            publicInputs: Self.sampleTonPublicInputs(targetDomain: sccpDomainSolana)
        ))) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("publicInputs.targetDomain"))
        }
    }

    func testTonBocRootHashBindsOrdinaryCells() throws {
        let boc = Data(hexString: "b5ee9c720101020100070001020101000202")!
        let checkedBoc = Data(hexString: "b5ee9c724101020100070001020101000202be1c1df5")!
        let rootHash = "0x49725ad44ef5ed5feaa27f88679cabae427209a6bea318cb9b66030131aae6fe"

        XCTAssertEqual(try tonBocRootHashes(boc), [rootHash])
        XCTAssertEqual(try tonBocSingleRootHash(boc), rootHash)
        XCTAssertEqual(try tonBocSingleRootHash(checkedBoc), rootHash)

        var badCrc = checkedBoc
        badCrc[badCrc.count - 1] ^= 0x01
        XCTAssertThrowsError(try tonBocSingleRootHash(badCrc))

        var changedChild = boc
        changedChild[changedChild.count - 1] ^= 0x01
        XCTAssertNotEqual(try tonBocSingleRootHash(changedChild), rootHash)

        var cyclicRef = boc
        cyclicRef[14] = 0
        XCTAssertThrowsError(try tonBocSingleRootHash(cyclicRef))

        var exoticCell = boc
        exoticCell[11] |= 0x08
        XCTAssertThrowsError(try tonBocSingleRootHash(exoticCell))

        var invalidPartialData = boc
        invalidPartialData[16] = 1
        invalidPartialData[17] = 0
        XCTAssertThrowsError(try tonBocSingleRootHash(invalidPartialData))

        let prunedBranchBoc = Data(hexString: "b5ee9c72010101010026002848010149725ad44ef5ed5feaa27f88679cabae427209a6bea318cb9b66030131aae6fe0001")!
        XCTAssertEqual(
            try tonBocSingleRootHash(prunedBranchBoc),
            "0xcc9095f882fb62a27bb19ad4aa84e19571a3283988ae40b75e238ad240cf1a96"
        )

        let legacyPrunedProofBoc = Data(hexString: "b5ee9c7201010601005f0022012001052201620203284801010bd445eea7213bd88307c204a267aa798c1bacb2ad2d781f6106a8296bc12b6500010103a0c0040004006f284801011d894d2390dbd75b607f99091580d9f1652f34c525e35ba648d4325cc7495d3e0001")!
        XCTAssertEqual(
            try tonBocSingleRootHash(legacyPrunedProofBoc),
            "0x9c769b035b601b0ddc098e9b148d9bdab0761c14bfe310ac090962ba1f39739a"
        )

        let merkleProofBoc = Data(hexString: "b5ee9c7201010301002d0009460349725ad44ef5ed5feaa27f88679cabae427209a6bea318cb9b66030131aae6fe00010101020102000202")!
        XCTAssertEqual(
            try tonBocSingleRootHash(merkleProofBoc),
            "0xe749bc5225cabbe3fa78fc12d74a734c365379bc0d302123dcf7bfa2ee3fbd21"
        )
        var mismatchedMerkleProof = merkleProofBoc
        mismatchedMerkleProof[14] ^= 0x01
        XCTAssertThrowsError(try tonBocSingleRootHash(mismatchedMerkleProof))

        let hashmapBoc = Data(hexString: "b5ee9c72010109010028000101c001020120020702016203050103a0c004000403090103a0c0060004006f0101de08000403e7")!
        let hashmapValueHash = "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419"
        XCTAssertEqual(
            try tonHashmapECellRefValueHash(hashmapBoc, key: Data([17]), keyBitLen: 8),
            hashmapValueHash
        )
        XCTAssertNil(try tonHashmapECellRefValueHash(hashmapBoc, key: Data([18]), keyBitLen: 8))
        XCTAssertThrowsError(try tonHashmapECellRefValueHash(hashmapBoc, key: Data([17]), keyBitLen: 7))

        let hashmapDirectProofBoc = Data(hexString: "b5ee9c72010107010063002101c00122012002062201620304284801010bd445eea7213bd88307c204a267aa798c1bacb2ad2d781f6106a8296bc12b6500010103a0c0050004006f284801011d894d2390dbd75b607f99091580d9f1652f34c525e35ba648d4325cc7495d3e0001")!
        XCTAssertEqual(
            try tonHashmapECellRefValueHash(hashmapDirectProofBoc, key: Data([17]), keyBitLen: 8),
            hashmapValueHash
        )
        XCTAssertNil(try tonHashmapECellRefValueHash(hashmapDirectProofBoc, key: Data([1]), keyBitLen: 8))

        let hashmapMerkleProofBoc = Data(hexString: "b5ee9c72010108010089000101c001094603e714f85374c2c336ed499a5a35e6c4f87441184532e7c23be795ce71b457f1bf00030222012003072201620405284801010bd445eea7213bd88307c204a267aa798c1bacb2ad2d781f6106a8296bc12b6500010103a0c0060004006f284801011d894d2390dbd75b607f99091580d9f1652f34c525e35ba648d4325cc7495d3e0001")!
        XCTAssertEqual(
            try tonHashmapECellRefValueHash(hashmapMerkleProofBoc, key: Data([17]), keyBitLen: 8),
            hashmapValueHash
        )

        let shardAccountsBoc = Data(hexString: "b5ee9c72010103010073000101c00101d37fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff84400000000000000000000000000000000000000000000000000000000000000005a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e41900000000000000078020000")!
        let shardAccountKey = Data(hexString: "1100000000000000000000000000000000000000000000000000000000000000")!
        let absentShardAccountKey = Data(hexString: "1200000000000000000000000000000000000000000000000000000000000000")!
        XCTAssertEqual(
            try tonShardAccountsLastTransactionHash(shardAccountsBoc, key: shardAccountKey, keyBitLen: 256),
            hashmapValueHash
        )
        XCTAssertEqual(
            try tonShardAccountsLastTransaction(shardAccountsBoc, key: shardAccountKey, keyBitLen: 256),
            TonShardAccountLastTransaction(hash: hashmapValueHash, lt: 7)
        )
        XCTAssertNil(try tonShardAccountsLastTransactionHash(shardAccountsBoc, key: absentShardAccountKey, keyBitLen: 256))
        XCTAssertThrowsError(try tonShardAccountsLastTransactionHash(shardAccountsBoc, key: Data([17]), keyBitLen: 8))

        let shardStateProofBoc = Data(hexString: "b5ee9c720101060100aa00035b9023afe2ffffff110000000000000000000000000000000007000000010000000b000000000000000c000000122001020500000101c00301d37fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff84400000000000000000000000000000000000000000000000000000000000000005a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419000000000000000780400000000")!
        XCTAssertEqual(
            try tonShardStateProofRootHash(shardStateProofBoc),
            "0x12a960855fea2f529c336d7325b1cca784f0f0b1a52ae149d02d046a2499e270"
        )
        XCTAssertEqual(
            try tonShardStateAccountsRootHash(shardStateProofBoc),
            "0x049a63ecefc78dc0cd468ebf47e0385807d790a2ca8e0dca5cbbeb0714567fd3"
        )
        var badShardStateTag = shardStateProofBoc
        let tagRange = try XCTUnwrap(badShardStateTag.range(of: Data([0x90, 0x23, 0xaf, 0xe2])))
        badShardStateTag[tagRange.lowerBound] ^= 0x01
        XCTAssertThrowsError(try tonShardStateAccountsRootHash(badShardStateTag))
        let shardIdentOffset = tagRange.lowerBound + 8
        var badShardIdentTag = shardStateProofBoc
        badShardIdentTag[shardIdentOffset] |= 0x80
        XCTAssertThrowsError(try tonShardStateAccountsRootHash(badShardIdentTag))
        var badShardIdentPrefixLen = shardStateProofBoc
        badShardIdentPrefixLen[shardIdentOffset] = 0x3d
        XCTAssertThrowsError(try tonShardStateAccountsRootHash(badShardIdentPrefixLen))
    }

    func testTonShardProofHashBindsWitnessMaterial() throws {
        let branch = [Data(repeating: 0xee, count: 32)]
        let shardStateBranch = [Data(repeating: 0x12, count: 32)]
        let bytes = try canonicalTonSccpShardProofBytes(
            sourceEventDigest: String(repeating: "34", count: 32),
            masterchainSeqno: 19,
            masterchainBlockHash: String(repeating: "aa", count: 32),
            shardWorkchainId: 0,
            shardShard: 0x8000_0000_0000_0000,
            shardSeqno: 7,
            shardBlockHash: String(repeating: "bb", count: 32),
            shardFileHash: String(repeating: "bc", count: 32),
            shardStateRoot: String(repeating: "cc", count: 32),
            transactionRoot: String(repeating: "dd", count: 32),
            transactionLt: 7,
            shardStateLeafIndex: 0,
            shardStateInclusionBranch: shardStateBranch,
            inclusionBranch: branch
        )

        XCTAssertEqual(bytes.count, 309)
        XCTAssertEqual(bytes.first, 1)

        let hash = try tonSccpShardProofHash(
            sourceEventDigest: String(repeating: "34", count: 32),
            masterchainSeqno: 19,
            masterchainBlockHash: String(repeating: "aa", count: 32),
            shardWorkchainId: 0,
            shardShard: 0x8000_0000_0000_0000,
            shardSeqno: 7,
            shardBlockHash: String(repeating: "bb", count: 32),
            shardFileHash: String(repeating: "bc", count: 32),
            shardStateRoot: String(repeating: "cc", count: 32),
            transactionRoot: String(repeating: "dd", count: 32),
            transactionLt: 7,
            shardStateLeafIndex: 0,
            shardStateInclusionBranch: shardStateBranch,
            inclusionBranch: branch
        )
        let changed = try tonSccpShardProofHash(
            sourceEventDigest: String(repeating: "34", count: 32),
            masterchainSeqno: 19,
            masterchainBlockHash: String(repeating: "aa", count: 32),
            shardWorkchainId: 0,
            shardShard: 0x8000_0000_0000_0000,
            shardSeqno: 7,
            shardBlockHash: String(repeating: "bb", count: 32),
            shardFileHash: String(repeating: "bc", count: 32),
            shardStateRoot: String(repeating: "cc", count: 32),
            transactionRoot: String(repeating: "dd", count: 32),
            transactionLt: 7,
            shardStateLeafIndex: 0,
            shardStateInclusionBranch: shardStateBranch,
            inclusionBranch: [Data(repeating: 0x12, count: 32)]
        )
        let changedShardState = try tonSccpShardProofHash(
            sourceEventDigest: String(repeating: "34", count: 32),
            masterchainSeqno: 19,
            masterchainBlockHash: String(repeating: "aa", count: 32),
            shardWorkchainId: 0,
            shardShard: 0x8000_0000_0000_0000,
            shardSeqno: 7,
            shardBlockHash: String(repeating: "bb", count: 32),
            shardFileHash: String(repeating: "bc", count: 32),
            shardStateRoot: String(repeating: "cc", count: 32),
            transactionRoot: String(repeating: "dd", count: 32),
            transactionLt: 7,
            shardStateLeafIndex: 0,
            shardStateInclusionBranch: [Data(repeating: 0xee, count: 32)],
            inclusionBranch: branch
        )
        XCTAssertEqual(hash, "0x09c63ca1185b537f0a37b7b248600a0992e5b7ed64ace9d1d437db7caae00686")
        XCTAssertNotEqual(hash, changed)
        XCTAssertNotEqual(hash, changedShardState)
        XCTAssertThrowsError(try canonicalTonSccpShardProofBytes(
            sourceEventDigest: String(repeating: "00", count: 32),
            masterchainSeqno: 19,
            masterchainBlockHash: String(repeating: "aa", count: 32),
            shardWorkchainId: 0,
            shardShard: 0x8000_0000_0000_0000,
            shardSeqno: 7,
            shardBlockHash: String(repeating: "bb", count: 32),
            shardFileHash: String(repeating: "bc", count: 32),
            shardStateRoot: String(repeating: "cc", count: 32),
            transactionRoot: String(repeating: "dd", count: 32),
            transactionLt: 7,
            shardStateLeafIndex: 0,
            shardStateInclusionBranch: shardStateBranch,
            inclusionBranch: branch
        )) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidHex32("sourceEventDigest"))
        }

        let hashmapBoc = Data(hexString: "b5ee9c72010103010073000101c00101d37fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff84400000000000000000000000000000000000000000000000000000000000000005a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e41900000000000000078020000")!
        let shardStateProofBoc = Data(hexString: "b5ee9c720101060100aa00035b9023afe2ffffff110000000000000000000000000000000007000000010000000b000000000000000c000000122001020500000101c00301d37fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff84400000000000000000000000000000000000000000000000000000000000000005a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419000000000000000780400000000")!
        let shardAccountKey = Data(hexString: "1100000000000000000000000000000000000000000000000000000000000000")!
        let shardStateRoot = "0x12a960855fea2f529c336d7325b1cca784f0f0b1a52ae149d02d046a2499e270"
        let shardAccountsRoot = "0x049a63ecefc78dc0cd468ebf47e0385807d790a2ca8e0dca5cbbeb0714567fd3"
        let dictionaryBytes = try canonicalTonSccpShardProofBytes(
            sourceEventDigest: String(repeating: "34", count: 32),
            masterchainSeqno: 19,
            masterchainBlockHash: String(repeating: "aa", count: 32),
            shardWorkchainId: 0,
            shardShard: 0x8000_0000_0000_0000,
            shardSeqno: 7,
            shardBlockHash: String(repeating: "bb", count: 32),
            shardFileHash: String(repeating: "bc", count: 32),
            shardStateRoot: shardStateRoot,
            transactionRoot: "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
            transactionLt: 7,
            shardStateLeafIndex: 0,
            shardStateInclusionBranch: [],
            inclusionBranch: branch,
            shardStateDictionaryRoot: shardAccountsRoot,
            shardStateDictionaryKeyBitLen: 256,
            shardStateDictionaryKey: shardAccountKey,
            shardStateDictionaryProofBoc: hashmapBoc,
            shardStateProofBoc: shardStateProofBoc
        )
        let dictionaryHash = try tonSccpShardProofHash(
            sourceEventDigest: String(repeating: "34", count: 32),
            masterchainSeqno: 19,
            masterchainBlockHash: String(repeating: "aa", count: 32),
            shardWorkchainId: 0,
            shardShard: 0x8000_0000_0000_0000,
            shardSeqno: 7,
            shardBlockHash: String(repeating: "bb", count: 32),
            shardFileHash: String(repeating: "bc", count: 32),
            shardStateRoot: shardStateRoot,
            transactionRoot: "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
            transactionLt: 7,
            shardStateLeafIndex: 0,
            shardStateInclusionBranch: [],
            inclusionBranch: branch,
            shardStateDictionaryRoot: shardAccountsRoot,
            shardStateDictionaryKeyBitLen: 256,
            shardStateDictionaryKey: shardAccountKey,
            shardStateDictionaryProofBoc: hashmapBoc,
            shardStateProofBoc: shardStateProofBoc
        )
        XCTAssertEqual(dictionaryBytes.count, 662)
        XCTAssertEqual(dictionaryHash, "0x32d8b496320e6a1ce5ccf671f2bd6f0d09cb53afed8c123b86cb9327b77c88cf")
        XCTAssertNotEqual(dictionaryHash, hash)
        XCTAssertThrowsError(try canonicalTonSccpShardProofBytes(
            sourceEventDigest: String(repeating: "34", count: 32),
            masterchainSeqno: 19,
            masterchainBlockHash: String(repeating: "aa", count: 32),
            shardWorkchainId: 0,
            shardShard: 0x8000_0000_0000_0000,
            shardSeqno: 7,
            shardBlockHash: String(repeating: "bb", count: 32),
            shardFileHash: String(repeating: "bc", count: 32),
            shardStateRoot: shardStateRoot,
            transactionRoot: "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
            transactionLt: 8,
            shardStateLeafIndex: 0,
            shardStateInclusionBranch: [],
            inclusionBranch: branch,
            shardStateDictionaryRoot: shardAccountsRoot,
            shardStateDictionaryKeyBitLen: 256,
            shardStateDictionaryKey: shardAccountKey,
            shardStateDictionaryProofBoc: hashmapBoc,
            shardStateProofBoc: shardStateProofBoc
        ))
        XCTAssertThrowsError(try canonicalTonSccpShardProofBytes(
            sourceEventDigest: String(repeating: "34", count: 32),
            masterchainSeqno: 19,
            masterchainBlockHash: String(repeating: "aa", count: 32),
            shardWorkchainId: 0,
            shardShard: 0x8000_0000_0000_0000,
            shardSeqno: 7,
            shardBlockHash: String(repeating: "bb", count: 32),
            shardFileHash: String(repeating: "bc", count: 32),
            shardStateRoot: shardStateRoot,
            transactionRoot: "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
            transactionLt: 7,
            shardStateLeafIndex: 0,
            shardStateInclusionBranch: shardStateBranch,
            inclusionBranch: branch,
            shardStateDictionaryRoot: shardAccountsRoot,
            shardStateDictionaryKeyBitLen: 256,
            shardStateDictionaryKey: shardAccountKey,
            shardStateDictionaryProofBoc: hashmapBoc,
            shardStateProofBoc: shardStateProofBoc
        ))
        var wrongGlobalIdProofBoc = shardStateProofBoc
        let wrongGlobalIdShardStateTag = Data([0x90, 0x23, 0xaf, 0xe2])
        guard let wrongGlobalIdTagRange = wrongGlobalIdProofBoc.range(of: wrongGlobalIdShardStateTag) else {
            XCTFail("test shard-state BoC contains ShardStateUnsplit tag")
            return
        }
        wrongGlobalIdProofBoc.replaceSubrange(
            (wrongGlobalIdTagRange.lowerBound + 4)..<(wrongGlobalIdTagRange.lowerBound + 8),
            with: Data(repeating: 0, count: 4)
        )
        XCTAssertEqual(try tonShardStateAccountsRootHash(wrongGlobalIdProofBoc), shardAccountsRoot)
        XCTAssertThrowsError(try canonicalTonSccpShardProofBytes(
            sourceEventDigest: String(repeating: "34", count: 32),
            masterchainSeqno: 19,
            masterchainBlockHash: String(repeating: "aa", count: 32),
            shardWorkchainId: 0,
            shardShard: 0x8000_0000_0000_0000,
            shardSeqno: 7,
            shardBlockHash: String(repeating: "bb", count: 32),
            shardFileHash: String(repeating: "bc", count: 32),
            shardStateRoot: try tonShardStateProofRootHash(wrongGlobalIdProofBoc),
            transactionRoot: "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
            transactionLt: 7,
            shardStateLeafIndex: 0,
            shardStateInclusionBranch: [],
            inclusionBranch: branch,
            shardStateDictionaryRoot: shardAccountsRoot,
            shardStateDictionaryKeyBitLen: 256,
            shardStateDictionaryKey: shardAccountKey,
            shardStateDictionaryProofBoc: hashmapBoc,
            shardStateProofBoc: wrongGlobalIdProofBoc
        ))
        var wrongWorkchainIdProofBoc = shardStateProofBoc
        guard let wrongWorkchainTagRange = wrongWorkchainIdProofBoc.range(of: wrongGlobalIdShardStateTag) else {
            XCTFail("test shard-state BoC contains ShardStateUnsplit tag")
            return
        }
        let wrongWorkchainShardIdentOffset = wrongWorkchainTagRange.lowerBound + 8
        wrongWorkchainIdProofBoc.replaceSubrange(
            (wrongWorkchainShardIdentOffset + 1)..<(wrongWorkchainShardIdentOffset + 5),
            with: Data(repeating: 0xff, count: 4)
        )
        XCTAssertEqual(try tonShardStateAccountsRootHash(wrongWorkchainIdProofBoc), shardAccountsRoot)
        XCTAssertThrowsError(try canonicalTonSccpShardProofBytes(
            sourceEventDigest: String(repeating: "34", count: 32),
            masterchainSeqno: 19,
            masterchainBlockHash: String(repeating: "aa", count: 32),
            shardWorkchainId: 0,
            shardShard: 0x8000_0000_0000_0000,
            shardSeqno: 7,
            shardBlockHash: String(repeating: "bb", count: 32),
            shardFileHash: String(repeating: "bc", count: 32),
            shardStateRoot: try tonShardStateProofRootHash(wrongWorkchainIdProofBoc),
            transactionRoot: "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
            transactionLt: 7,
            shardStateLeafIndex: 0,
            shardStateInclusionBranch: [],
            inclusionBranch: branch,
            shardStateDictionaryRoot: shardAccountsRoot,
            shardStateDictionaryKeyBitLen: 256,
            shardStateDictionaryKey: shardAccountKey,
            shardStateDictionaryProofBoc: hashmapBoc,
            shardStateProofBoc: wrongWorkchainIdProofBoc
        ))
        var zeroGenUtimeProofBoc = shardStateProofBoc
        guard let zeroGenUtimeTagRange = zeroGenUtimeProofBoc.range(of: wrongGlobalIdShardStateTag) else {
            XCTFail("test shard-state BoC contains ShardStateUnsplit tag")
            return
        }
        zeroGenUtimeProofBoc.replaceSubrange(
            (zeroGenUtimeTagRange.lowerBound + 29)..<(zeroGenUtimeTagRange.lowerBound + 33),
            with: Data(repeating: 0, count: 4)
        )
        XCTAssertThrowsError(try canonicalTonSccpShardProofBytes(
            sourceEventDigest: String(repeating: "34", count: 32),
            masterchainSeqno: 19,
            masterchainBlockHash: String(repeating: "aa", count: 32),
            shardWorkchainId: 0,
            shardShard: 0x8000_0000_0000_0000,
            shardSeqno: 7,
            shardBlockHash: String(repeating: "bb", count: 32),
            shardFileHash: String(repeating: "bc", count: 32),
            shardStateRoot: try tonShardStateProofRootHash(zeroGenUtimeProofBoc),
            transactionRoot: "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
            transactionLt: 7,
            shardStateLeafIndex: 0,
            shardStateInclusionBranch: [],
            inclusionBranch: branch,
            shardStateDictionaryRoot: shardAccountsRoot,
            shardStateDictionaryKeyBitLen: 256,
            shardStateDictionaryKey: shardAccountKey,
            shardStateDictionaryProofBoc: hashmapBoc,
            shardStateProofBoc: zeroGenUtimeProofBoc
        ))
        var futureMinRefMcSeqnoProofBoc = shardStateProofBoc
        guard let futureMinRefMcSeqnoTagRange = futureMinRefMcSeqnoProofBoc.range(of: wrongGlobalIdShardStateTag) else {
            XCTFail("test shard-state BoC contains ShardStateUnsplit tag")
            return
        }
        futureMinRefMcSeqnoProofBoc[futureMinRefMcSeqnoTagRange.lowerBound + 44] = 0x14
        XCTAssertThrowsError(try canonicalTonSccpShardProofBytes(
            sourceEventDigest: String(repeating: "34", count: 32),
            masterchainSeqno: 19,
            masterchainBlockHash: String(repeating: "aa", count: 32),
            shardWorkchainId: 0,
            shardShard: 0x8000_0000_0000_0000,
            shardSeqno: 7,
            shardBlockHash: String(repeating: "bb", count: 32),
            shardFileHash: String(repeating: "bc", count: 32),
            shardStateRoot: try tonShardStateProofRootHash(futureMinRefMcSeqnoProofBoc),
            transactionRoot: "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
            transactionLt: 7,
            shardStateLeafIndex: 0,
            shardStateInclusionBranch: [],
            inclusionBranch: branch,
            shardStateDictionaryRoot: shardAccountsRoot,
            shardStateDictionaryKeyBitLen: 256,
            shardStateDictionaryKey: shardAccountKey,
            shardStateDictionaryProofBoc: hashmapBoc,
            shardStateProofBoc: futureMinRefMcSeqnoProofBoc
        ))
        var basechainCustomProofBoc = shardStateProofBoc
        guard let basechainCustomTagRange = basechainCustomProofBoc.range(of: wrongGlobalIdShardStateTag) else {
            XCTFail("test shard-state BoC contains ShardStateUnsplit tag")
            return
        }
        basechainCustomProofBoc[basechainCustomTagRange.lowerBound + 45] |= 0x40
        XCTAssertThrowsError(try tonShardStateAccountsRootHash(basechainCustomProofBoc))
        XCTAssertThrowsError(try canonicalTonSccpShardProofBytes(
            sourceEventDigest: String(repeating: "34", count: 32),
            masterchainSeqno: 19,
            masterchainBlockHash: String(repeating: "aa", count: 32),
            shardWorkchainId: 0,
            shardShard: 0x8000_0000_0000_0000,
            shardSeqno: 7,
            shardBlockHash: String(repeating: "bb", count: 32),
            shardFileHash: String(repeating: "bc", count: 32),
            shardStateRoot: try tonShardStateProofRootHash(basechainCustomProofBoc),
            transactionRoot: "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
            transactionLt: 7,
            shardStateLeafIndex: 0,
            shardStateInclusionBranch: [],
            inclusionBranch: branch,
            shardStateDictionaryRoot: shardAccountsRoot,
            shardStateDictionaryKeyBitLen: 256,
            shardStateDictionaryKey: shardAccountKey,
            shardStateDictionaryProofBoc: hashmapBoc,
            shardStateProofBoc: basechainCustomProofBoc
        ))
        var mismatchedShardPrefixProofBoc = shardStateProofBoc
        let shardStateTag = Data([0x90, 0x23, 0xaf, 0xe2])
        guard let shardStateTagRange = mismatchedShardPrefixProofBoc.range(of: shardStateTag) else {
            XCTFail("test shard-state BoC contains ShardStateUnsplit tag")
            return
        }
        let shardIdentOffset = shardStateTagRange.lowerBound + 8
        mismatchedShardPrefixProofBoc[shardIdentOffset] = 0x08
        mismatchedShardPrefixProofBoc[shardIdentOffset + 5] = 0x12
        XCTAssertEqual(try tonShardStateAccountsRootHash(mismatchedShardPrefixProofBoc), shardAccountsRoot)
        XCTAssertThrowsError(try canonicalTonSccpShardProofBytes(
            sourceEventDigest: String(repeating: "34", count: 32),
            masterchainSeqno: 19,
            masterchainBlockHash: String(repeating: "aa", count: 32),
            shardWorkchainId: 0,
            shardShard: 0x1280_0000_0000_0000,
            shardSeqno: 7,
            shardBlockHash: String(repeating: "bb", count: 32),
            shardFileHash: String(repeating: "bc", count: 32),
            shardStateRoot: try tonShardStateProofRootHash(mismatchedShardPrefixProofBoc),
            transactionRoot: "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
            transactionLt: 7,
            shardStateLeafIndex: 0,
            shardStateInclusionBranch: [],
            inclusionBranch: branch,
            shardStateDictionaryRoot: shardAccountsRoot,
            shardStateDictionaryKeyBitLen: 256,
            shardStateDictionaryKey: shardAccountKey,
            shardStateDictionaryProofBoc: hashmapBoc,
            shardStateProofBoc: mismatchedShardPrefixProofBoc
        ))
        XCTAssertThrowsError(try canonicalTonSccpShardProofBytes(
            sourceEventDigest: String(repeating: "34", count: 32),
            masterchainSeqno: 19,
            masterchainBlockHash: String(repeating: "aa", count: 32),
            shardWorkchainId: 0,
            shardShard: 0x8000_0000_0000_0000,
            shardSeqno: 7,
            shardBlockHash: String(repeating: "bb", count: 32),
            shardFileHash: String(repeating: "bc", count: 32),
            shardStateRoot: String(repeating: "66", count: 32),
            transactionRoot: "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
            transactionLt: 7,
            shardStateLeafIndex: 0,
            shardStateInclusionBranch: [],
            inclusionBranch: branch,
            shardStateDictionaryRoot: shardAccountsRoot,
            shardStateDictionaryKeyBitLen: 256,
            shardStateDictionaryKey: shardAccountKey,
            shardStateDictionaryProofBoc: hashmapBoc,
            shardStateProofBoc: shardStateProofBoc
        ))
        XCTAssertThrowsError(try canonicalTonSccpShardProofBytes(
            sourceEventDigest: String(repeating: "34", count: 32),
            masterchainSeqno: 19,
            masterchainBlockHash: String(repeating: "aa", count: 32),
            shardWorkchainId: 0,
            shardShard: 0x8000_0000_0000_0000,
            shardSeqno: 7,
            shardBlockHash: String(repeating: "bb", count: 32),
            shardFileHash: String(repeating: "bc", count: 32),
            shardStateRoot: shardStateRoot,
            transactionRoot: String(repeating: "66", count: 32),
            transactionLt: 7,
            shardStateLeafIndex: 0,
            shardStateInclusionBranch: [],
            inclusionBranch: branch,
            shardStateDictionaryRoot: shardAccountsRoot,
            shardStateDictionaryKeyBitLen: 256,
            shardStateDictionaryKey: shardAccountKey,
            shardStateDictionaryProofBoc: hashmapBoc,
            shardStateProofBoc: shardStateProofBoc
        ))
        XCTAssertThrowsError(try canonicalTonSccpShardProofBytes(
            sourceEventDigest: String(repeating: "34", count: 32),
            masterchainSeqno: 19,
            masterchainBlockHash: String(repeating: "aa", count: 32),
            shardWorkchainId: 0,
            shardShard: 0x8000_0000_0000_0000,
            shardSeqno: 7,
            shardBlockHash: String(repeating: "bb", count: 32),
            shardFileHash: String(repeating: "bc", count: 32),
            shardStateRoot: shardStateRoot,
            transactionRoot: "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
            transactionLt: 7,
            shardStateLeafIndex: 0,
            shardStateInclusionBranch: [],
            inclusionBranch: branch,
            shardStateDictionaryRoot: String(repeating: "66", count: 32),
            shardStateDictionaryKeyBitLen: 256,
            shardStateDictionaryKey: shardAccountKey,
            shardStateDictionaryProofBoc: hashmapBoc,
            shardStateProofBoc: shardStateProofBoc
        ))
        XCTAssertThrowsError(try canonicalTonSccpShardProofBytes(
            sourceEventDigest: String(repeating: "34", count: 32),
            masterchainSeqno: 19,
            masterchainBlockHash: String(repeating: "aa", count: 32),
            shardWorkchainId: 0,
            shardShard: 0x8000_0000_0000_0000,
            shardSeqno: 7,
            shardBlockHash: String(repeating: "bb", count: 32),
            shardFileHash: String(repeating: "bc", count: 32),
            shardStateRoot: shardStateRoot,
            transactionRoot: "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
            transactionLt: 7,
            shardStateLeafIndex: 0,
            shardStateInclusionBranch: shardStateBranch,
            inclusionBranch: branch,
            shardStateDictionaryRoot: String(repeating: "00", count: 32),
            shardStateDictionaryKeyBitLen: 256,
            shardStateDictionaryKey: shardAccountKey,
            shardStateDictionaryProofBoc: hashmapBoc,
            shardStateProofBoc: shardStateProofBoc
        ))
        XCTAssertThrowsError(try canonicalTonSccpShardProofBytes(
            sourceEventDigest: String(repeating: "34", count: 32),
            masterchainSeqno: 19,
            masterchainBlockHash: String(repeating: "aa", count: 32),
            shardWorkchainId: 0,
            shardShard: 0x8000_0000_0000_0000,
            shardSeqno: 7,
            shardBlockHash: String(repeating: "bb", count: 32),
            shardFileHash: String(repeating: "bc", count: 32),
            shardStateRoot: shardStateRoot,
            transactionRoot: "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
            transactionLt: 7,
            shardStateLeafIndex: 0,
            shardStateInclusionBranch: shardStateBranch,
            inclusionBranch: branch,
            shardStateDictionaryRoot: shardAccountsRoot,
            shardStateDictionaryKeyBitLen: 7,
            shardStateDictionaryKey: Data([17]),
            shardStateDictionaryProofBoc: hashmapBoc,
            shardStateProofBoc: shardStateProofBoc
        ))
        XCTAssertThrowsError(try canonicalTonSccpShardProofBytes(
            sourceEventDigest: String(repeating: "34", count: 32),
            masterchainSeqno: 19,
            masterchainBlockHash: String(repeating: "aa", count: 32),
            shardWorkchainId: 0,
            shardShard: 0x8000_0000_0000_0000,
            shardSeqno: 7,
            shardBlockHash: String(repeating: "bb", count: 32),
            shardFileHash: String(repeating: "bc", count: 32),
            shardStateRoot: String(repeating: "cc", count: 32),
            transactionRoot: String(repeating: "dd", count: 32),
            transactionLt: 7,
            shardStateLeafIndex: 0,
            shardStateInclusionBranch: shardStateBranch,
            inclusionBranch: [Data([1, 2, 3])]
        ))
        XCTAssertThrowsError(try canonicalTonSccpShardProofBytes(
            sourceEventDigest: String(repeating: "34", count: 32),
            masterchainSeqno: 19,
            masterchainBlockHash: String(repeating: "aa", count: 32),
            shardWorkchainId: 0,
            shardShard: 0x8000_0000_0000_0000,
            shardSeqno: 7,
            shardBlockHash: String(repeating: "bb", count: 32),
            shardFileHash: String(repeating: "bc", count: 32),
            shardStateRoot: String(repeating: "cc", count: 32),
            transactionRoot: String(repeating: "dd", count: 32),
            transactionLt: 7,
            shardStateLeafIndex: 0,
            shardStateInclusionBranch: [Data([1, 2, 3])],
            inclusionBranch: branch
        ))
        XCTAssertThrowsError(try canonicalTonSccpShardProofBytes(
            sourceEventDigest: String(repeating: "34", count: 32),
            masterchainSeqno: 19,
            masterchainBlockHash: String(repeating: "aa", count: 32),
            shardWorkchainId: 0,
            shardShard: 0x8000_0000_0000_0000,
            shardSeqno: 7,
            shardBlockHash: String(repeating: "bb", count: 32),
            shardFileHash: String(repeating: "bc", count: 32),
            shardStateRoot: String(repeating: "cc", count: 32),
            transactionRoot: String(repeating: "dd", count: 32),
            transactionLt: 7,
            shardStateLeafIndex: 0,
            shardStateInclusionBranch: shardStateBranch,
            inclusionBranch: Array(repeating: Data(repeating: 0xee, count: 32), count: 65)
        ))
    }

    func testTonShardStateOpenVerifyProofRequestBindsWitnessMaterial() throws {
        let input = Self.sampleTonShardStateProofRequestInput()
        let statement = try canonicalTonShardStateProofPublicInputsBytes(input)
        let witness = try canonicalTonShardStateWitnessCommitmentBytes(input)
        let context = try canonicalTonShardStateVerificationContextBytes(input)
        let schema = try tonShardStateOpenVerifySchemaDescriptor(input)
        let publicInputsHash = try tonShardStateProofPublicInputsHash(input)
        let columns = try tonShardStatePublicInputColumns(input)
        let request = try buildTonShardStateProofRequest(input)

        XCTAssertEqual(statement.count, 603)
        XCTAssertEqual(publicInputsHash, "0x82bdedb87242c4bb073b7c97cb339b7f1300e3692e327c5bc8233bd105cafb19")
        XCTAssertEqual(witness.count, 480)
        XCTAssertEqual(context.count, 467)
        XCTAssertEqual(schema.count, 436)
        XCTAssertEqual(request.circuitId, sccpTonShardStateOpenVerifyCircuitIdV1)
        XCTAssertEqual(request.proofFamily, "stark-fri-v1")
        XCTAssertEqual(request.fastpqPublicInputs.dsid, "0x27e44edc7d124906a8176e94557996c3")
        XCTAssertEqual(request.fastpqPublicInputs.txSetHash, publicInputsHash)
        XCTAssertEqual(request.shardStateProofPublicInputsHash, publicInputsHash)
        XCTAssertEqual(columns[15][0], publicInputsHash)
        XCTAssertEqual(request.publicInputColumns[15][0], publicInputsHash)
        XCTAssertEqual(request.fastpqTransitions[0].key, "sccp:ton:shard-state:v1:statement")
        XCTAssertEqual(request.fastpqTransitions[1].key, "sccp:ton:shard-state:v1:witness")
        XCTAssertEqual(request.fastpqTransitions[2].key, "sccp:ton:shard-state:v1:context")
        XCTAssertEqual(request.statementBytes, statement)
        XCTAssertEqual(request.witnessCommitmentBytes, witness)
        XCTAssertEqual(request.verificationContextBytes, context)
        XCTAssertEqual(request.schemaDescriptor, schema)
        let transitionProof = try Self.sampleTonValidatorSetTransitionProofInput()
        let transitionBoundInput = Self.sampleTonShardStateProofRequestInput(
            validatorSetTransitionProofs: [transitionProof]
        )
        var tamperedSignatures = transitionProof.validatorSignatureProof.signatures
        tamperedSignatures[0][tamperedSignatures[0].startIndex] = 0xaa
        let tamperedTransitionProof = try Self.sampleTonValidatorSetTransitionProofInput(
            signatures: tamperedSignatures
        )
        XCTAssertNotEqual(
            try canonicalTonShardStateProofPublicInputsBytes(transitionBoundInput),
            try canonicalTonShardStateProofPublicInputsBytes(
                Self.sampleTonShardStateProofRequestInput(
                    validatorSetTransitionProofs: [tamperedTransitionProof]
                )
            )
        )
        var exposedShardStatement = request.statementBytes
        exposedShardStatement[exposedShardStatement.startIndex] =
            exposedShardStatement[exposedShardStatement.startIndex] == 0 ? 1 : 0
        XCTAssertEqual(request.statementBytes, statement)
        var exposedShardWitness = request.witnessCommitmentBytes
        exposedShardWitness[exposedShardWitness.startIndex] =
            exposedShardWitness[exposedShardWitness.startIndex] == 0 ? 1 : 0
        XCTAssertEqual(request.witnessCommitmentBytes, witness)
        XCTAssertThrowsError(try buildTonShardStateProofRequest(
            Self.sampleTonShardStateProofRequestInput(
                sourceStateVerifierHash: "0x540205f876591604ccf39f72a051ac5e82647c9e48dbd48cb129d2543971a34f"
            )
        )) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("sourceStateVerifierHash"))
        }

        let mismatched = Self.sampleTonShardStateProofRequestInput(transactionRoot: String(repeating: "66", count: 32))
        XCTAssertThrowsError(try tonShardStateProofPublicInputsHash(mismatched))
    }

    func testTonFullLightClientAuditRoleProofRequests() async throws {
        let input = try Self.sampleTonFullLightClientAuditProofInput()
        func replacing(
            _ input: TonSccpFullLightClientAuditProofInput,
            masterchainConfigVerifierHash: String? = nil,
            validatorSetTransitionVerifierHash: String? = nil,
            shardStateVerificationProofHash: String? = nil
        ) -> TonSccpFullLightClientAuditProofInput {
            TonSccpFullLightClientAuditProofInput(
                shardState: input.shardState,
                shardStateVerificationProof: input.shardStateVerificationProof,
                validatorSetPayloadHash: input.validatorSetPayloadHash,
                configLeafHash: input.configLeafHash,
                configValueHash: input.configValueHash,
                sourceVerifierMaterialHash: input.sourceVerifierMaterialHash,
                sourceAdapterDeploymentHash: input.sourceAdapterDeploymentHash,
                fullLightClientGateHash: input.fullLightClientGateHash,
                tonMasterchainConfigVerifierHash: masterchainConfigVerifierHash
                    ?? input.tonMasterchainConfigVerifierHash,
                tonValidatorSetTransitionVerifierHash: validatorSetTransitionVerifierHash
                    ?? input.tonValidatorSetTransitionVerifierHash,
                tonShardAccountsDictionaryVerifierHash: input.tonShardAccountsDictionaryVerifierHash,
                shardStateProofPublicInputsHash: input.shardStateProofPublicInputsHash,
                shardStateVerificationProofHash: shardStateVerificationProofHash
                    ?? input.shardStateVerificationProofHash
            )
        }
        let requests = try buildTonSccpFullLightClientAuditProofRequests(input)
        let shardStateProofPublicInputsHash = try tonShardStateProofPublicInputsHash(input.shardState)
        let shardStateVerificationProofHash = try tonSccpShardStateVerificationProofHash(
            input.shardStateVerificationProof
        )
        XCTAssertThrowsError(
            try canonicalTonSccpSourceStateVerificationProofBytes(
                TonSccpSourceStateVerificationProof(
                    version: 0,
                    proofBytes: Data([1, 2, 3])
                )
            )
        ) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("sourceStateVerificationProof"))
        }

        XCTAssertEqual(requests.masterchainConfig.circuitId, sccpTonMasterchainConfigOpenVerifyCircuitIdV1)
        XCTAssertEqual(
            requests.validatorSetTransition.circuitId,
            sccpTonValidatorSetTransitionOpenVerifyCircuitIdV1
        )
        XCTAssertEqual(
            requests.shardAccountsDictionary.circuitId,
            sccpTonShardAccountsDictionaryOpenVerifyCircuitIdV1
        )
        XCTAssertEqual(Set([
            requests.masterchainConfig.circuitId,
            requests.validatorSetTransition.circuitId,
            requests.shardAccountsDictionary.circuitId,
        ]).count, 3)
        XCTAssertEqual(
            [
                requests.masterchainConfig.role,
                requests.validatorSetTransition.role,
                requests.shardAccountsDictionary.role,
            ],
            ["masterchain_config", "validator_set_transition", "shard_accounts_dictionary"]
        )
        XCTAssertGreaterThan(
            try canonicalTonSccpFullLightClientAuditStatementBytes(input, role: .masterchainConfig).count,
            0
        )
        for request in [
            requests.masterchainConfig,
            requests.validatorSetTransition,
            requests.shardAccountsDictionary,
        ] {
            let role: TonSccpFullLightClientAuditRole
            switch request.roleCode {
            case 1:
                role = .masterchainConfig
            case 2:
                role = .validatorSetTransition
            default:
                role = .shardAccountsDictionary
            }
            XCTAssertEqual(request.version, 1)
            XCTAssertEqual(request.proofFamily, "stark-fri-v1")
            XCTAssertEqual(request.parameterSet, "fastpq-lane-balanced")
            XCTAssertEqual(request.sourceDomain, sccpDomainTon)
            XCTAssertEqual(request.masterchainSeqno, "19")
            XCTAssertEqual(request.shardSeqno, "7")
            XCTAssertEqual(request.sourceStateVerifierId, sccpTonMainnetShardStateVerifierIdV1)
            XCTAssertEqual(request.fullLightClientGateHash, input.fullLightClientGateHash)
            XCTAssertEqual(request.shardStateProofPublicInputsHash, shardStateProofPublicInputsHash)
            XCTAssertEqual(request.shardStateVerificationProofHash, shardStateVerificationProofHash)
            XCTAssertEqual(request.fastpqTransitions.count, 3)
            XCTAssertEqual(request.fastpqTransitions.map(\.key), request.fastpqTransitions.map(\.key).sorted())
            XCTAssertTrue(request.fastpqTransitions.allSatisfy { $0.key.hasPrefix("0x") })
            XCTAssertEqual(
                request.auditStatementHash,
                try tonSccpFullLightClientAuditStatementHash(input, role: role)
            )
        }
        XCTAssertEqual(requests.masterchainConfig.publicInputColumns.count, 17)
        XCTAssertEqual(requests.validatorSetTransition.publicInputColumns.count, 16)
        XCTAssertEqual(requests.shardAccountsDictionary.publicInputColumns.count, 17)
        XCTAssertEqual(
            requests.masterchainConfig.publicInputColumns,
            try tonSccpFullLightClientAuditPublicInputColumns(input, role: .masterchainConfig)
        )
        XCTAssertEqual(
            requests.masterchainConfig.schemaDescriptor,
            try tonSccpFullLightClientAuditOpenVerifySchemaDescriptor(input, role: .masterchainConfig)
        )
        var exposedAuditStatement = requests.masterchainConfig.statementBytes
        exposedAuditStatement[exposedAuditStatement.startIndex] =
            exposedAuditStatement[exposedAuditStatement.startIndex] == 0 ? 1 : 0
        XCTAssertEqual(
            requests.masterchainConfig.statementBytes,
            try canonicalTonSccpFullLightClientAuditStatementBytes(input, role: .masterchainConfig)
        )
        var exposedAuditSchema = requests.shardAccountsDictionary.schemaDescriptor
        exposedAuditSchema[exposedAuditSchema.startIndex] =
            exposedAuditSchema[exposedAuditSchema.startIndex] == 0 ? 1 : 0
        XCTAssertEqual(
            requests.shardAccountsDictionary.schemaDescriptor,
            try tonSccpFullLightClientAuditOpenVerifySchemaDescriptor(input, role: .shardAccountsDictionary)
        )
        XCTAssertEqual(requests.masterchainConfig.fastpqPublicInputs.oldRoot, input.shardState.masterchainConfigRoot)
        XCTAssertEqual(
            requests.validatorSetTransition.fastpqPublicInputs.oldRoot,
            input.shardState.sourceTrustAnchorHash
        )
        XCTAssertEqual(requests.shardAccountsDictionary.fastpqPublicInputs.newRoot, input.shardState.transactionRoot)

        let shardRequest = try buildTonShardStateProofRequest(input.shardState)
        let wrappedShard = try wrapTonSccpSourceStateVerificationProof(
            proofBytes: Data([9, 8, 7]),
            request: shardRequest
        )
        XCTAssertEqual(wrappedShard.circuitId, sccpTonShardStateOpenVerifyCircuitIdV1)
        XCTAssertEqual(wrappedShard.proofBytes, Data([9, 8, 7]))
        XCTAssertEqual(wrappedShard.proofBase64, "CQgH")
        var exposedTonProofBytes = wrappedShard.proofBytes
        exposedTonProofBytes[0] = 0
        XCTAssertEqual(wrappedShard.proofBytes, Data([9, 8, 7]))
        XCTAssertEqual(wrappedShard.proofBase64, "CQgH")
        XCTAssertGreaterThan(try canonicalTonSccpSourceStateVerificationProofBytes(wrappedShard).count, 0)
        let wrappedAudit = try wrapTonSccpSourceStateVerificationProof(
            proofBytes: Data([1, 2, 3]),
            request: requests.masterchainConfig
        )
        XCTAssertEqual(wrappedAudit.circuitId, sccpTonMasterchainConfigOpenVerifyCircuitIdV1)
        XCTAssertEqual(wrappedAudit.proofBase64, "AQID")
        XCTAssertGreaterThan(try canonicalTonSccpSourceStateVerificationProofBytes(wrappedAudit).count, 0)
        XCTAssertThrowsError(
            try wrapTonSccpSourceStateVerificationProof(
                proofBytes: Data(repeating: 0, count: 2),
                request: shardRequest
            )
        ) { error in
            XCTAssertEqual(error as? TonSccpProverError, .allZeroProof)
        }
        var tamperedShardTransitions = shardRequest.fastpqTransitions
        tamperedShardTransitions[0] = TonShardStateFastpqTransition(
            key: tamperedShardTransitions[0].key,
            operation: tamperedShardTransitions[0].operation,
            oldValue: tamperedShardTransitions[0].oldValue,
            newValue: "0x00"
        )
        let tamperedShardRequest = TonShardStateProofRequest(
            version: shardRequest.version,
            proofFamily: shardRequest.proofFamily,
            circuitId: shardRequest.circuitId,
            parameterSet: shardRequest.parameterSet,
            sourceDomain: shardRequest.sourceDomain,
            masterchainSeqno: shardRequest.masterchainSeqno,
            shardSeqno: shardRequest.shardSeqno,
            sourceStateVerifierId: shardRequest.sourceStateVerifierId,
            sourceStateVerifierHash: shardRequest.sourceStateVerifierHash,
            shardStateProofPublicInputsHash: shardRequest.shardStateProofPublicInputsHash,
            statementBytes: shardRequest.statementBytes,
            witnessCommitmentBytes: shardRequest.witnessCommitmentBytes,
            verificationContextBytes: shardRequest.verificationContextBytes,
            schemaDescriptor: shardRequest.schemaDescriptor,
            publicInputColumns: shardRequest.publicInputColumns,
            fastpqPublicInputs: shardRequest.fastpqPublicInputs,
            fastpqTransitions: tamperedShardTransitions
        )
        XCTAssertThrowsError(try wrapTonSccpSourceStateVerificationProof(
            proofBytes: Data([9, 8, 7]),
            request: tamperedShardRequest
        )) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("request.fastpqTransitions"))
        }
        let tamperedShardFastpqInputs = TonShardStateFastpqPublicInputs(
            dsid: "0x" + String(repeating: "00", count: 16),
            slot: shardRequest.fastpqPublicInputs.slot,
            oldRoot: shardRequest.fastpqPublicInputs.oldRoot,
            newRoot: shardRequest.fastpqPublicInputs.newRoot,
            permRoot: shardRequest.fastpqPublicInputs.permRoot,
            txSetHash: shardRequest.fastpqPublicInputs.txSetHash
        )
        let tamperedShardDsidRequest = TonShardStateProofRequest(
            version: shardRequest.version,
            proofFamily: shardRequest.proofFamily,
            circuitId: shardRequest.circuitId,
            parameterSet: shardRequest.parameterSet,
            sourceDomain: shardRequest.sourceDomain,
            masterchainSeqno: shardRequest.masterchainSeqno,
            shardSeqno: shardRequest.shardSeqno,
            sourceStateVerifierId: shardRequest.sourceStateVerifierId,
            sourceStateVerifierHash: shardRequest.sourceStateVerifierHash,
            shardStateProofPublicInputsHash: shardRequest.shardStateProofPublicInputsHash,
            statementBytes: shardRequest.statementBytes,
            witnessCommitmentBytes: shardRequest.witnessCommitmentBytes,
            verificationContextBytes: shardRequest.verificationContextBytes,
            schemaDescriptor: shardRequest.schemaDescriptor,
            publicInputColumns: shardRequest.publicInputColumns,
            fastpqPublicInputs: tamperedShardFastpqInputs,
            fastpqTransitions: shardRequest.fastpqTransitions
        )
        XCTAssertThrowsError(try wrapTonSccpSourceStateVerificationProof(
            proofBytes: Data([9, 8, 7]),
            request: tamperedShardDsidRequest
        )) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("request.fastpqPublicInputs.dsid"))
        }
        var tamperedAuditTransitions = requests.masterchainConfig.fastpqTransitions
        tamperedAuditTransitions[0] = TonSccpFullLightClientAuditFastpqTransition(
            key: tamperedAuditTransitions[0].key,
            operation: tamperedAuditTransitions[0].operation,
            oldValue: tamperedAuditTransitions[0].oldValue,
            newValue: "0x00"
        )
        let tamperedAuditRequest = TonSccpFullLightClientAuditProofRequest(
            version: requests.masterchainConfig.version,
            proofFamily: requests.masterchainConfig.proofFamily,
            circuitId: requests.masterchainConfig.circuitId,
            parameterSet: requests.masterchainConfig.parameterSet,
            role: requests.masterchainConfig.role,
            roleCode: requests.masterchainConfig.roleCode,
            sourceDomain: requests.masterchainConfig.sourceDomain,
            masterchainSeqno: requests.masterchainConfig.masterchainSeqno,
            shardSeqno: requests.masterchainConfig.shardSeqno,
            verifierId: requests.masterchainConfig.verifierId,
            verifierHash: requests.masterchainConfig.verifierHash,
            sourceStateVerifierId: requests.masterchainConfig.sourceStateVerifierId,
            sourceStateVerifierHash: requests.masterchainConfig.sourceStateVerifierHash,
            sourceVerifierMaterialHash: requests.masterchainConfig.sourceVerifierMaterialHash,
            sourceAdapterDeploymentHash: requests.masterchainConfig.sourceAdapterDeploymentHash,
            fullLightClientGateHash: requests.masterchainConfig.fullLightClientGateHash,
            shardStateProofPublicInputsHash: requests.masterchainConfig.shardStateProofPublicInputsHash,
            shardStateVerificationProofHash: requests.masterchainConfig.shardStateVerificationProofHash,
            auditStatementHash: requests.masterchainConfig.auditStatementHash,
            statementBytes: requests.masterchainConfig.statementBytes,
            verificationContextBytes: requests.masterchainConfig.verificationContextBytes,
            schemaDescriptor: requests.masterchainConfig.schemaDescriptor,
            publicInputColumns: requests.masterchainConfig.publicInputColumns,
            fastpqPublicInputs: requests.masterchainConfig.fastpqPublicInputs,
            fastpqTransitions: tamperedAuditTransitions
        )
        XCTAssertThrowsError(try wrapTonSccpSourceStateVerificationProof(
            proofBytes: Data([9, 8, 7]),
            request: tamperedAuditRequest
        )) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("request.fastpqTransitions"))
        }
        let tamperedAuditFastpqInputs = TonSccpFullLightClientAuditFastpqPublicInputs(
            dsid: requests.masterchainConfig.fastpqPublicInputs.dsid,
            slot: requests.masterchainConfig.fastpqPublicInputs.slot,
            oldRoot: requests.masterchainConfig.fastpqPublicInputs.oldRoot,
            newRoot: requests.masterchainConfig.fastpqPublicInputs.newRoot,
            permRoot: requests.masterchainConfig.fastpqPublicInputs.permRoot,
            txSetHash: "0x" + String(repeating: "aa", count: 32)
        )
        let tamperedAuditTxRequest = TonSccpFullLightClientAuditProofRequest(
            version: requests.masterchainConfig.version,
            proofFamily: requests.masterchainConfig.proofFamily,
            circuitId: requests.masterchainConfig.circuitId,
            parameterSet: requests.masterchainConfig.parameterSet,
            role: requests.masterchainConfig.role,
            roleCode: requests.masterchainConfig.roleCode,
            sourceDomain: requests.masterchainConfig.sourceDomain,
            masterchainSeqno: requests.masterchainConfig.masterchainSeqno,
            shardSeqno: requests.masterchainConfig.shardSeqno,
            verifierId: requests.masterchainConfig.verifierId,
            verifierHash: requests.masterchainConfig.verifierHash,
            sourceStateVerifierId: requests.masterchainConfig.sourceStateVerifierId,
            sourceStateVerifierHash: requests.masterchainConfig.sourceStateVerifierHash,
            sourceVerifierMaterialHash: requests.masterchainConfig.sourceVerifierMaterialHash,
            sourceAdapterDeploymentHash: requests.masterchainConfig.sourceAdapterDeploymentHash,
            fullLightClientGateHash: requests.masterchainConfig.fullLightClientGateHash,
            shardStateProofPublicInputsHash: requests.masterchainConfig.shardStateProofPublicInputsHash,
            shardStateVerificationProofHash: requests.masterchainConfig.shardStateVerificationProofHash,
            auditStatementHash: requests.masterchainConfig.auditStatementHash,
            statementBytes: requests.masterchainConfig.statementBytes,
            verificationContextBytes: requests.masterchainConfig.verificationContextBytes,
            schemaDescriptor: requests.masterchainConfig.schemaDescriptor,
            publicInputColumns: requests.masterchainConfig.publicInputColumns,
            fastpqPublicInputs: tamperedAuditFastpqInputs,
            fastpqTransitions: requests.masterchainConfig.fastpqTransitions
        )
        XCTAssertThrowsError(try wrapTonSccpSourceStateVerificationProof(
            proofBytes: Data([9, 8, 7]),
            request: tamperedAuditTxRequest
        )) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("request.fastpqPublicInputs.txSetHash"))
        }

        var preflightCallbackInvoked = false
        let preflightCheckingProver = TonSccpSourceStateProver(
            shardStateProveFunction: { _ in
                preflightCallbackInvoked = true
                return Data([9, 8, 7])
            },
            fullLightClientAuditProveFunction: { _ in
                preflightCallbackInvoked = true
                return Data([9, 8, 7])
            }
        )
        do {
            _ = try await preflightCheckingProver.proveShardState(request: tamperedShardRequest)
            XCTFail("expected malformed TON shard-state request to fail before callback")
        } catch {
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("request.fastpqTransitions"))
        }
        XCTAssertFalse(preflightCallbackInvoked)
        do {
            _ = try await preflightCheckingProver.proveFullLightClientAudit(request: tamperedAuditRequest)
            XCTFail("expected malformed TON audit request to fail before callback")
        } catch {
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("request.fastpqTransitions"))
        }
        XCTAssertFalse(preflightCallbackInvoked)

        var seenTonRoles: [String] = []
        var seenTonShardCallbackRequest: TonShardStateProofRequest?
        var seenTonAuditCallbackRequests: [TonSccpFullLightClientAuditProofRequest] = []
        let tonSourceStateProver = TonSccpSourceStateProver(
            shardStateProveFunction: { request in
                seenTonRoles.append("shard_state")
                seenTonShardCallbackRequest = request
                XCTAssertEqual(request.circuitId, sccpTonShardStateOpenVerifyCircuitIdV1)
                XCTAssertEqual(request, shardRequest)
                var callbackStatementBytes = request.statementBytes
                callbackStatementBytes[callbackStatementBytes.startIndex] =
                    callbackStatementBytes[callbackStatementBytes.startIndex] == 0 ? 1 : 0
                XCTAssertEqual(request.statementBytes, shardRequest.statementBytes)
                var callbackColumns = request.publicInputColumns
                callbackColumns[0][0] = "0x00"
                XCTAssertEqual(request.publicInputColumns, shardRequest.publicInputColumns)
                return Data([9, 8, 7])
            },
            fullLightClientAuditProveFunction: { request in
                let expectedAuditRequests = [
                    requests.masterchainConfig,
                    requests.validatorSetTransition,
                    requests.shardAccountsDictionary,
                ]
                let expectedRequest = expectedAuditRequests[seenTonAuditCallbackRequests.count]
                seenTonRoles.append(request.role)
                seenTonAuditCallbackRequests.append(request)
                XCTAssertEqual(request, expectedRequest)
                var callbackStatementBytes = request.statementBytes
                callbackStatementBytes[callbackStatementBytes.startIndex] =
                    callbackStatementBytes[callbackStatementBytes.startIndex] == 0 ? 1 : 0
                XCTAssertEqual(request.statementBytes, expectedRequest.statementBytes)
                var callbackColumns = request.publicInputColumns
                callbackColumns[0][0] = "0x00"
                XCTAssertEqual(request.publicInputColumns, expectedRequest.publicInputColumns)
                return Data([9, 8, 7])
            }
        )
        let linkedShardProof = try await tonSourceStateProver.proveShardState(input.shardState)
        let linkedTonProofs = try await tonSourceStateProver.proveFullLightClientAudit(input)
        XCTAssertEqual(linkedShardProof.circuitId, sccpTonShardStateOpenVerifyCircuitIdV1)
        XCTAssertEqual(linkedShardProof.proofBase64, "CQgH")
        XCTAssertEqual(
            seenTonRoles,
            ["shard_state", "masterchain_config", "validator_set_transition", "shard_accounts_dictionary"]
        )
        XCTAssertEqual(seenTonShardCallbackRequest, shardRequest)
        XCTAssertEqual(
            seenTonAuditCallbackRequests,
            [requests.masterchainConfig, requests.validatorSetTransition, requests.shardAccountsDictionary]
        )
        XCTAssertEqual(
            linkedTonProofs.validatorSetTransition.circuitId,
            sccpTonValidatorSetTransitionOpenVerifyCircuitIdV1
        )
        XCTAssertEqual(
            linkedTonProofs.shardAccountsDictionary.circuitId,
            sccpTonShardAccountsDictionaryOpenVerifyCircuitIdV1
        )
        XCTAssertEqual(linkedTonProofs.shardAccountsDictionary.proofBytes, Data([9, 8, 7]))
        XCTAssertEqual(linkedTonProofs.shardAccountsDictionary.proofBase64, "CQgH")
        do {
            _ = try await TonSccpSourceStateProver().proveFullLightClientAudit(input)
            XCTFail("expected missing TON source-state prover")
        } catch {
            XCTAssertEqual(error as? TonSccpProverError, .localProverUnavailable)
        }

        XCTAssertThrowsError(try buildTonSccpFullLightClientAuditProofRequests(
            replacing(input, validatorSetTransitionVerifierHash: "0x" + String(repeating: "b1", count: 32))
        )) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("tonAuditVerifierHash"))
        }
        XCTAssertThrowsError(try buildTonSccpFullLightClientAuditProofRequests(
            replacing(input, masterchainConfigVerifierHash: "0x" + String(repeating: "d4", count: 32))
        )) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("tonAuditVerifierHash"))
        }
        XCTAssertThrowsError(try buildTonSccpFullLightClientAuditProofRequests(
            replacing(
                input,
                masterchainConfigVerifierHash: try tonSccpFullLightClientAuditStatementHash(
                    input,
                    role: .masterchainConfig
                )
            )
        )) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("tonAuditVerifierHash"))
        }
        XCTAssertThrowsError(try buildTonSccpFullLightClientAuditProofRequests(
            replacing(
                input,
                masterchainConfigVerifierHash: "0xd83b3a3eb920ac8338533535cf0d6c69c69d507e84aef8ec2094564b8427c56c"
            )
        )) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("tonAuditVerifierHash"))
        }
        XCTAssertThrowsError(try buildTonSccpFullLightClientAuditProofRequests(
            replacing(input, shardStateVerificationProofHash: "0x" + String(repeating: "aa", count: 32))
        )) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("shardStateVerificationProofHash"))
        }
        XCTAssertThrowsError(try buildTonSccpFullLightClientAuditProofRequests(
            try Self.sampleTonFullLightClientAuditProofInput(masterchainConfigProofHash: "0x" + String(repeating: "aa", count: 32))
        )) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("masterchainConfigProofHash"))
        }
    }

    func testTonValidatorSetTransitionHashesBindWitnessMaterial() throws {
        let validatorPublicKeys = [Data(repeating: 0x11, count: 32), Data(repeating: 0x22, count: 32)]
        let validatorWeights: [UInt64] = [1, 2]
        let validatorSetHash = "0x68bfccd52bc19cf8cdaffc611d58e53824d0aee395a4d813eca0bcefb3970938"
        let nextValidatorPublicKeys = [Data(repeating: 0x33, count: 32), Data(repeating: 0x44, count: 32)]
        let nextValidatorWeights: [UInt64] = [3, 4]
        let nextValidatorSetPayload = try canonicalTonValidatorSetPayloadBytes(
            validatorPublicKeys: nextValidatorPublicKeys,
            validatorWeights: nextValidatorWeights
        )
        let nextValidatorSetHash = "0x26bfcffe8913e5e4f09e56076d5a237cbc5b890d31b8912bd7eacc5d3805691f"
        let nextValidatorSetPayloadHash = "0xb76b843e99596a049425653e9921e4227af23a5b70331940fa057f1f58314983"
        let transitionMessageHash = "0x91eda926884eb1ae700e7b398c46f6d47fbb973efa322564894936140ccd2a19"
        let transitionSignatureHash = "0xd784461f68495981c2c00e60316dc9353ea4b5be3bc261b26feadc7c83c4f6a7"
        let signatureProof = TonValidatorSignatureProofInput(
            totalWeight: 3,
            signedWeight: 3,
            blockMessageHash: transitionMessageHash,
            validatorPublicKeys: validatorPublicKeys,
            validatorWeights: validatorWeights,
            signersBitmap: Data([0x03]),
            signatures: [Data(repeating: 0xab, count: 64), Data(repeating: 0xcd, count: 64)]
        )

        XCTAssertEqual(
            try canonicalTonValidatorSetBytes(
                validatorPublicKeys: validatorPublicKeys,
                validatorWeights: validatorWeights
            ).count,
            85
        )
        XCTAssertEqual(
            try tonValidatorSetHash(
                validatorPublicKeys: validatorPublicKeys,
                validatorWeights: validatorWeights
            ),
            validatorSetHash
        )
        XCTAssertEqual(try tonValidatorSetPayloadHash(payload: nextValidatorSetPayload), nextValidatorSetPayloadHash)
        XCTAssertEqual(try tonValidatorSetHashFromPayload(payload: nextValidatorSetPayload), nextValidatorSetHash)
        XCTAssertEqual(
            try canonicalTonValidatorSetTransitionMessageBytes(
                sourceDomain: sccpDomainTon,
                fromValidatorSetSeqno: 7,
                toValidatorSetSeqno: 8,
                masterchainSeqno: 19,
                masterchainWorkchainId: -1,
                masterchainShard: 0x8000_0000_0000_0000,
                masterchainBlockHash: String(repeating: "aa", count: 32),
                masterchainFileHash: String(repeating: "a5", count: 32),
                parentValidatorSetHash: validatorSetHash,
                nextValidatorSetHash: nextValidatorSetHash,
                nextValidatorSetPayloadHash: nextValidatorSetPayloadHash,
                nextValidatorSetConfigHash: String(repeating: "cc", count: 32)
            ).count,
            233
        )
        XCTAssertEqual(
            try tonValidatorSetTransitionMessageHash(
                sourceDomain: sccpDomainTon,
                fromValidatorSetSeqno: 7,
                toValidatorSetSeqno: 8,
                masterchainSeqno: 19,
                masterchainWorkchainId: -1,
                masterchainShard: 0x8000_0000_0000_0000,
                masterchainBlockHash: String(repeating: "aa", count: 32),
                masterchainFileHash: String(repeating: "a5", count: 32),
                parentValidatorSetHash: validatorSetHash,
                nextValidatorSetHash: nextValidatorSetHash,
                nextValidatorSetPayloadHash: nextValidatorSetPayloadHash,
                nextValidatorSetConfigHash: String(repeating: "cc", count: 32)
            ),
            transitionMessageHash
        )
        XCTAssertEqual(
            try canonicalTonValidatorSetTransitionSignatureBytes(
                sourceDomain: sccpDomainTon,
                fromValidatorSetSeqno: 7,
                toValidatorSetSeqno: 8,
                masterchainSeqno: 19,
                masterchainWorkchainId: -1,
                masterchainShard: 0x8000_0000_0000_0000,
                masterchainBlockHash: String(repeating: "aa", count: 32),
                masterchainFileHash: String(repeating: "a5", count: 32),
                parentValidatorSetHash: validatorSetHash,
                nextValidatorSetHash: nextValidatorSetHash,
                nextValidatorSetPayload: nextValidatorSetPayload,
                nextValidatorSetPayloadHash: nextValidatorSetPayloadHash,
                nextValidatorSetConfigHash: String(repeating: "cc", count: 32),
                transitionMessageHash: transitionMessageHash,
                validatorSignatureProof: signatureProof
            ).count,
            676
        )
        XCTAssertEqual(
            try tonValidatorSetTransitionSignatureHash(
                sourceDomain: sccpDomainTon,
                fromValidatorSetSeqno: 7,
                toValidatorSetSeqno: 8,
                masterchainSeqno: 19,
                masterchainWorkchainId: -1,
                masterchainShard: 0x8000_0000_0000_0000,
                masterchainBlockHash: String(repeating: "aa", count: 32),
                masterchainFileHash: String(repeating: "a5", count: 32),
                parentValidatorSetHash: validatorSetHash,
                nextValidatorSetHash: nextValidatorSetHash,
                nextValidatorSetPayload: nextValidatorSetPayload,
                nextValidatorSetPayloadHash: nextValidatorSetPayloadHash,
                nextValidatorSetConfigHash: String(repeating: "cc", count: 32),
                transitionMessageHash: transitionMessageHash,
                validatorSignatureProof: signatureProof
            ),
            transitionSignatureHash
        )
        XCTAssertThrowsError(try canonicalTonValidatorSetTransitionSignatureBytes(
            sourceDomain: sccpDomainTon,
            fromValidatorSetSeqno: 7,
            toValidatorSetSeqno: 8,
            masterchainSeqno: 19,
            masterchainWorkchainId: -1,
            masterchainShard: 0x8000_0000_0000_0000,
            masterchainBlockHash: String(repeating: "aa", count: 32),
            masterchainFileHash: String(repeating: "a5", count: 32),
            parentValidatorSetHash: String(repeating: "dd", count: 32),
            nextValidatorSetHash: nextValidatorSetHash,
            nextValidatorSetPayload: nextValidatorSetPayload,
            nextValidatorSetPayloadHash: nextValidatorSetPayloadHash,
            nextValidatorSetConfigHash: String(repeating: "cc", count: 32),
            transitionMessageHash: transitionMessageHash,
            validatorSignatureProof: signatureProof
        ))
        XCTAssertThrowsError(try canonicalTonValidatorSetTransitionSignatureBytes(
            sourceDomain: sccpDomainTon,
            fromValidatorSetSeqno: 7,
            toValidatorSetSeqno: 8,
            masterchainSeqno: 19,
            masterchainWorkchainId: -1,
            masterchainShard: 0x8000_0000_0000_0000,
            masterchainBlockHash: String(repeating: "aa", count: 32),
            masterchainFileHash: String(repeating: "a5", count: 32),
            parentValidatorSetHash: validatorSetHash,
            nextValidatorSetHash: nextValidatorSetHash,
            nextValidatorSetPayload: nextValidatorSetPayload,
            nextValidatorSetPayloadHash: nextValidatorSetPayloadHash,
            nextValidatorSetConfigHash: String(repeating: "cc", count: 32),
            transitionMessageHash: String(repeating: "dd", count: 32),
            validatorSignatureProof: signatureProof
        ))
        XCTAssertThrowsError(try canonicalTonValidatorSetTransitionMessageBytes(
            sourceDomain: sccpDomainTon,
            fromValidatorSetSeqno: 7,
            toValidatorSetSeqno: 9,
            masterchainSeqno: 19,
            masterchainWorkchainId: -1,
            masterchainShard: 0x8000_0000_0000_0000,
            masterchainBlockHash: String(repeating: "aa", count: 32),
            masterchainFileHash: String(repeating: "a5", count: 32),
            parentValidatorSetHash: validatorSetHash,
            nextValidatorSetHash: nextValidatorSetHash,
            nextValidatorSetPayloadHash: nextValidatorSetPayloadHash,
            nextValidatorSetConfigHash: String(repeating: "cc", count: 32)
        ))
        XCTAssertThrowsError(try canonicalTonValidatorSetTransitionSignatureBytes(
            sourceDomain: sccpDomainTon,
            fromValidatorSetSeqno: 7,
            toValidatorSetSeqno: 8,
            masterchainSeqno: 19,
            masterchainWorkchainId: -1,
            masterchainShard: 0x8000_0000_0000_0000,
            masterchainBlockHash: String(repeating: "aa", count: 32),
            masterchainFileHash: String(repeating: "a5", count: 32),
            parentValidatorSetHash: validatorSetHash,
            nextValidatorSetHash: nextValidatorSetHash,
            nextValidatorSetPayload: nextValidatorSetPayload,
            nextValidatorSetPayloadHash: nextValidatorSetPayloadHash,
            nextValidatorSetConfigHash: String(repeating: "cc", count: 32),
            transitionMessageHash: transitionMessageHash,
            validatorSignatureProof: TonValidatorSignatureProofInput(
                totalWeight: 3,
                signedWeight: 3,
                blockMessageHash: String(repeating: "dd", count: 32),
                validatorPublicKeys: validatorPublicKeys,
                validatorWeights: validatorWeights,
                signersBitmap: Data([0x03]),
                signatures: [Data(repeating: 0xab, count: 64), Data(repeating: 0xcd, count: 64)]
            )
        ))
        XCTAssertThrowsError(try canonicalTonValidatorSetBytes(
            validatorPublicKeys: validatorPublicKeys,
            validatorWeights: [1, 0]
        ))
        XCTAssertThrowsError(try canonicalTonValidatorSetBytes(
            validatorPublicKeys: [Data(repeating: 0, count: 32), validatorPublicKeys[1]],
            validatorWeights: validatorWeights
        ))
        var zeroKeyValidatorSetPayload = try canonicalTonValidatorSetPayloadBytes(
            validatorPublicKeys: validatorPublicKeys,
            validatorWeights: validatorWeights
        )
        zeroKeyValidatorSetPayload.replaceSubrange(5 ..< 37, with: Data(repeating: 0, count: 32))
        XCTAssertThrowsError(try tonValidatorSetHashFromPayload(payload: zeroKeyValidatorSetPayload))
        let oversizedValidatorPublicKeys = (0 ... 1024).map { index -> Data in
            var publicKey = Data(repeating: 0, count: 32)
            publicKey[0] = 0x80
            var encodedIndex = UInt32(index).littleEndian
            withUnsafeBytes(of: &encodedIndex) { bytes in
                publicKey.replaceSubrange(28 ..< 32, with: bytes)
            }
            return publicKey
        }
        XCTAssertThrowsError(try canonicalTonValidatorSetBytes(
            validatorPublicKeys: oversizedValidatorPublicKeys,
            validatorWeights: Array(repeating: 1, count: oversizedValidatorPublicKeys.count)
        ))
        var oversizedValidatorSetPayload = Data([1])
        var oversizedCount = UInt32(1025).littleEndian
        withUnsafeBytes(of: &oversizedCount) { oversizedValidatorSetPayload.append(contentsOf: $0) }
        for publicKey in oversizedValidatorPublicKeys {
            oversizedValidatorSetPayload.append(publicKey)
            var weight = UInt64(1).littleEndian
            withUnsafeBytes(of: &weight) { oversizedValidatorSetPayload.append(contentsOf: $0) }
        }
        XCTAssertThrowsError(try tonValidatorSetHashFromPayload(payload: oversizedValidatorSetPayload))
        XCTAssertThrowsError(try canonicalTonValidatorSetTransitionSignatureBytes(
            sourceDomain: sccpDomainTon,
            fromValidatorSetSeqno: 7,
            toValidatorSetSeqno: 8,
            masterchainSeqno: 19,
            masterchainWorkchainId: -1,
            masterchainShard: 0x8000_0000_0000_0000,
            masterchainBlockHash: String(repeating: "aa", count: 32),
            masterchainFileHash: String(repeating: "a5", count: 32),
            parentValidatorSetHash: validatorSetHash,
            nextValidatorSetHash: nextValidatorSetHash,
            nextValidatorSetPayload: nextValidatorSetPayload,
            nextValidatorSetPayloadHash: nextValidatorSetPayloadHash,
            nextValidatorSetConfigHash: String(repeating: "cc", count: 32),
            transitionMessageHash: transitionMessageHash,
            validatorSignatureProof: TonValidatorSignatureProofInput(
                totalWeight: 3,
                signedWeight: 3,
                blockMessageHash: transitionMessageHash,
                validatorPublicKeys: validatorPublicKeys,
                validatorWeights: validatorWeights,
                signersBitmap: Data([0x03]),
                signatures: [Data(repeating: 0xab, count: 64)]
            )
        ))
        XCTAssertThrowsError(try canonicalTonValidatorSetTransitionSignatureBytes(
            sourceDomain: sccpDomainTon,
            fromValidatorSetSeqno: 7,
            toValidatorSetSeqno: 8,
            masterchainSeqno: 19,
            masterchainWorkchainId: -1,
            masterchainShard: 0x8000_0000_0000_0000,
            masterchainBlockHash: String(repeating: "aa", count: 32),
            masterchainFileHash: String(repeating: "a5", count: 32),
            parentValidatorSetHash: validatorSetHash,
            nextValidatorSetHash: nextValidatorSetHash,
            nextValidatorSetPayload: nextValidatorSetPayload,
            nextValidatorSetPayloadHash: nextValidatorSetPayloadHash,
            nextValidatorSetConfigHash: String(repeating: "cc", count: 32),
            transitionMessageHash: transitionMessageHash,
            validatorSignatureProof: TonValidatorSignatureProofInput(
                totalWeight: 3,
                signedWeight: 3,
                blockMessageHash: transitionMessageHash,
                validatorPublicKeys: validatorPublicKeys,
                validatorWeights: validatorWeights,
                signersBitmap: Data([0x03]),
                signatures: [Data(repeating: 0xab, count: 63), Data(repeating: 0xcd, count: 64)]
            )
        ))
        XCTAssertThrowsError(try canonicalTonValidatorSetTransitionSignatureBytes(
            sourceDomain: sccpDomainTon,
            fromValidatorSetSeqno: 7,
            toValidatorSetSeqno: 8,
            masterchainSeqno: 19,
            masterchainWorkchainId: -1,
            masterchainShard: 0x8000_0000_0000_0000,
            masterchainBlockHash: String(repeating: "aa", count: 32),
            masterchainFileHash: String(repeating: "a5", count: 32),
            parentValidatorSetHash: validatorSetHash,
            nextValidatorSetHash: nextValidatorSetHash,
            nextValidatorSetPayload: nextValidatorSetPayload,
            nextValidatorSetPayloadHash: nextValidatorSetPayloadHash,
            nextValidatorSetConfigHash: String(repeating: "cc", count: 32),
            transitionMessageHash: transitionMessageHash,
            validatorSignatureProof: TonValidatorSignatureProofInput(
                totalWeight: 3,
                signedWeight: 3,
                blockMessageHash: transitionMessageHash,
                validatorPublicKeys: validatorPublicKeys,
                validatorWeights: validatorWeights,
                signersBitmap: Data([0x03]),
                signatures: [Data(repeating: 0, count: 64), Data(repeating: 0x01, count: 64)]
            )
        ))
        XCTAssertThrowsError(try canonicalTonValidatorSetTransitionSignatureBytes(
            sourceDomain: sccpDomainTon,
            fromValidatorSetSeqno: 7,
            toValidatorSetSeqno: 8,
            masterchainSeqno: 19,
            masterchainWorkchainId: -1,
            masterchainShard: 0x8000_0000_0000_0000,
            masterchainBlockHash: String(repeating: "aa", count: 32),
            masterchainFileHash: String(repeating: "a5", count: 32),
            parentValidatorSetHash: validatorSetHash,
            nextValidatorSetHash: nextValidatorSetHash,
            nextValidatorSetPayload: nextValidatorSetPayload,
            nextValidatorSetPayloadHash: nextValidatorSetPayloadHash,
            nextValidatorSetConfigHash: String(repeating: "cc", count: 32),
            transitionMessageHash: transitionMessageHash,
            validatorSignatureProof: TonValidatorSignatureProofInput(
                totalWeight: 3,
                signedWeight: 1,
                blockMessageHash: transitionMessageHash,
                validatorPublicKeys: validatorPublicKeys,
                validatorWeights: validatorWeights,
                signersBitmap: Data([0x01]),
                signatures: [Data(repeating: 0xab, count: 64)]
            )
        ))
    }

    func testTonMasterchainConfigProofHashesBindWitnessMaterial() throws {
        let validatorPublicKeys = [Data(repeating: 0x11, count: 32), Data(repeating: 0x22, count: 32)]
        let validatorWeights: [UInt64] = [1, 2]
        let validatorSetHash = "0x68bfccd52bc19cf8cdaffc611d58e53824d0aee395a4d813eca0bcefb3970938"
        let validatorSetPayloadHash = "0xb322afe2faa070a2ed88a922c5ac5d27e5f9fecc41a11ffbed37cca293c4aeb0"
        let configLeafHash = "0xed92ba8082850092da7cc296a2184cc4576877aaee08c72748d96ea449b16e39"
        let configProofBoc = Data(hexString: "b5ee9c72010106010091000101c00101117fffffff80000008a002012b120000000100000002000200020000000000000003c00302087fff00000405005b14e3a049e28444444444444444444444444444444444444444444444444444444444444444400000000000000060005b14e3a049e288888888888888888888888888888888888888888888888888888888888888888000000000000000a0")!
        let configRoot = "0x5bf87008e0e76085d6db977b53a89329de49a4eed8fd1ff90d8c78f096ef05af"
        let configValueHash = "0x1aa64eb5ca0b3cb254dfada709904ce81f8b327eed0d83f2522122a0a9dddd50"
        let configProofHash = "0x9949285613a9e9dfb4ed3728bbede7ddea36fd82ac3d7eff3955dd75e9c4941c"
        let validatorSetPayload = try canonicalTonValidatorSetPayloadBytes(
            validatorPublicKeys: validatorPublicKeys,
            validatorWeights: validatorWeights
        )

        XCTAssertEqual(try tonValidatorSetPayloadHash(payload: validatorSetPayload), validatorSetPayloadHash)
        XCTAssertEqual(try tonConfigValidatorSetPayloadFromProofBoc(configProofBoc), validatorSetPayload)
        XCTAssertEqual(
            try tonConfigValidatorSetPayloadHashFromProofBoc(configProofBoc),
            validatorSetPayloadHash
        )
        XCTAssertEqual(try tonHashmapEProofRootHash(configProofBoc), configRoot)
        XCTAssertEqual(
            try tonHashmapECellRefValueHash(
                configProofBoc,
                key: Data([0, 0, 0, UInt8(sccpTonCurrentValidatorSetConfigParam)]),
                keyBitLen: sccpTonConfigParamKeyBits
            ),
            configValueHash
        )
        XCTAssertEqual(
            try canonicalTonMasterchainConfigLeafBytes(
                sourceDomain: sccpDomainTon,
                masterchainSeqno: 19,
                masterchainBlockHash: String(repeating: "aa", count: 32),
                shardStateRoot: String(repeating: "cc", count: 32),
                validatorSetHash: validatorSetHash,
                validatorSetPayloadHash: validatorSetPayloadHash
            ).count,
            141
        )
        XCTAssertThrowsError(try canonicalTonMasterchainConfigLeafBytes(
            version: 0,
            sourceDomain: sccpDomainTon,
            masterchainSeqno: 19,
            masterchainBlockHash: String(repeating: "aa", count: 32),
            shardStateRoot: String(repeating: "cc", count: 32),
            validatorSetHash: validatorSetHash,
            validatorSetPayloadHash: validatorSetPayloadHash
        ))
        XCTAssertEqual(
            try tonMasterchainConfigLeafHash(
                sourceDomain: sccpDomainTon,
                masterchainSeqno: 19,
                masterchainBlockHash: String(repeating: "aa", count: 32),
                shardStateRoot: String(repeating: "cc", count: 32),
                validatorSetHash: validatorSetHash,
                validatorSetPayloadHash: validatorSetPayloadHash
            ),
            configLeafHash
        )
        XCTAssertEqual(
            try canonicalTonMasterchainConfigProofBytes(
                sourceDomain: sccpDomainTon,
                masterchainSeqno: 19,
                masterchainBlockHash: String(repeating: "aa", count: 32),
                shardStateRoot: String(repeating: "cc", count: 32),
                configRoot: configRoot,
                validatorSetHash: validatorSetHash,
                validatorSetPayloadHash: validatorSetPayloadHash,
                configLeafHash: configLeafHash,
                configLeafIndex: sccpTonCurrentValidatorSetConfigParam,
                configValueHash: configValueHash,
                configDictionaryProofBoc: configProofBoc,
                configInclusionBranch: []
            ).count,
            411
        )
        XCTAssertEqual(
            try tonMasterchainConfigProofHash(
                sourceDomain: sccpDomainTon,
                masterchainSeqno: 19,
                masterchainBlockHash: String(repeating: "aa", count: 32),
                shardStateRoot: String(repeating: "cc", count: 32),
                configRoot: configRoot,
                validatorSetHash: validatorSetHash,
                validatorSetPayloadHash: validatorSetPayloadHash,
                configLeafHash: configLeafHash,
                configLeafIndex: sccpTonCurrentValidatorSetConfigParam,
                configValueHash: configValueHash,
                configDictionaryProofBoc: configProofBoc,
                configInclusionBranch: []
            ),
            configProofHash
        )
        XCTAssertThrowsError(try canonicalTonMasterchainConfigProofBytes(
            sourceDomain: sccpDomainTon,
            masterchainSeqno: 19,
            masterchainBlockHash: String(repeating: "aa", count: 32),
            shardStateRoot: String(repeating: "cc", count: 32),
            configRoot: configRoot,
            validatorSetHash: validatorSetHash,
            validatorSetPayloadHash: validatorSetPayloadHash,
            configLeafHash: configLeafHash,
            configLeafIndex: 0,
            configValueHash: configValueHash,
            configDictionaryProofBoc: configProofBoc,
            configInclusionBranch: []
        ))
        XCTAssertThrowsError(try canonicalTonMasterchainConfigProofBytes(
            sourceDomain: sccpDomainTon,
            masterchainSeqno: 19,
            masterchainBlockHash: String(repeating: "aa", count: 32),
            shardStateRoot: String(repeating: "cc", count: 32),
            configRoot: configRoot,
            validatorSetHash: validatorSetHash,
            validatorSetPayloadHash: validatorSetPayloadHash,
            configLeafHash: String(repeating: "ee", count: 32),
            configLeafIndex: sccpTonCurrentValidatorSetConfigParam,
            configValueHash: configValueHash,
            configDictionaryProofBoc: configProofBoc,
            configInclusionBranch: []
        ))
        let wrongValidatorSetHash = String(repeating: "ee", count: 32)
        let wrongValidatorSetLeafHash = try tonMasterchainConfigLeafHash(
            sourceDomain: sccpDomainTon,
            masterchainSeqno: 19,
            masterchainBlockHash: String(repeating: "aa", count: 32),
            shardStateRoot: String(repeating: "cc", count: 32),
            validatorSetHash: wrongValidatorSetHash,
            validatorSetPayloadHash: validatorSetPayloadHash
        )
        XCTAssertThrowsError(try canonicalTonMasterchainConfigProofBytes(
            sourceDomain: sccpDomainTon,
            masterchainSeqno: 19,
            masterchainBlockHash: String(repeating: "aa", count: 32),
            shardStateRoot: String(repeating: "cc", count: 32),
            configRoot: configRoot,
            validatorSetHash: wrongValidatorSetHash,
            validatorSetPayloadHash: validatorSetPayloadHash,
            configLeafHash: wrongValidatorSetLeafHash,
            configLeafIndex: sccpTonCurrentValidatorSetConfigParam,
            configValueHash: configValueHash,
            configDictionaryProofBoc: configProofBoc,
            configInclusionBranch: []
        ))
        XCTAssertThrowsError(try canonicalTonMasterchainConfigProofBytes(
            sourceDomain: sccpDomainSolana,
            masterchainSeqno: 19,
            masterchainBlockHash: String(repeating: "aa", count: 32),
            shardStateRoot: String(repeating: "cc", count: 32),
            configRoot: configRoot,
            validatorSetHash: validatorSetHash,
            validatorSetPayloadHash: validatorSetPayloadHash,
            configLeafHash: configLeafHash,
            configLeafIndex: sccpTonCurrentValidatorSetConfigParam,
            configValueHash: configValueHash,
            configDictionaryProofBoc: configProofBoc,
            configInclusionBranch: []
        ))
        XCTAssertThrowsError(try canonicalTonMasterchainConfigProofBytes(
            sourceDomain: sccpDomainTon,
            masterchainSeqno: 19,
            masterchainBlockHash: String(repeating: "aa", count: 32),
            shardStateRoot: String(repeating: "cc", count: 32),
            configRoot: configRoot,
            validatorSetHash: validatorSetHash,
            validatorSetPayloadHash: String(repeating: "ee", count: 32),
            configLeafHash: configLeafHash,
            configLeafIndex: sccpTonCurrentValidatorSetConfigParam,
            configValueHash: configValueHash,
            configDictionaryProofBoc: configProofBoc,
            configInclusionBranch: []
        ))
        XCTAssertThrowsError(try canonicalTonMasterchainConfigProofBytes(
            sourceDomain: sccpDomainTon,
            masterchainSeqno: 19,
            masterchainBlockHash: String(repeating: "aa", count: 32),
            shardStateRoot: String(repeating: "cc", count: 32),
            configRoot: configRoot,
            validatorSetHash: validatorSetHash,
            validatorSetPayloadHash: validatorSetPayloadHash,
            configLeafHash: configLeafHash,
            configLeafIndex: sccpTonCurrentValidatorSetConfigParam,
            configValueHash: String(repeating: "ee", count: 32),
            configDictionaryProofBoc: configProofBoc,
            configInclusionBranch: []
        ))
        XCTAssertThrowsError(try canonicalTonMasterchainConfigProofBytes(
            sourceDomain: sccpDomainTon,
            masterchainSeqno: 19,
            masterchainBlockHash: String(repeating: "aa", count: 32),
            shardStateRoot: String(repeating: "cc", count: 32),
            configRoot: configRoot,
            validatorSetHash: validatorSetHash,
            validatorSetPayloadHash: validatorSetPayloadHash,
            configLeafHash: configLeafHash,
            configLeafIndex: sccpTonCurrentValidatorSetConfigParam,
            configValueHash: configValueHash,
            configDictionaryProofBoc: configProofBoc,
            configInclusionBranch: [Data(repeating: 0xee, count: 32)]
        ))
    }

    func testTonMasterchainBlockMessageAndSignaturesBindWitnessMaterial() throws {
        let validatorPublicKeys = [Data(repeating: 0x11, count: 32), Data(repeating: 0x22, count: 32)]
        let validatorWeights: [UInt64] = [1, 2]
        let validatorSetHash = "0x68bfccd52bc19cf8cdaffc611d58e53824d0aee395a4d813eca0bcefb3970938"
        let configRoot = "0x5bf87008e0e76085d6db977b53a89329de49a4eed8fd1ff90d8c78f096ef05af"
        let configProofHash = "0x9949285613a9e9dfb4ed3728bbede7ddea36fd82ac3d7eff3955dd75e9c4941c"
        let blockMessageHash = "0x0ca07d5072adb7db3d6a0f831294c7e119c451884aaa1afcbb23e0df0911d8bd"
        let signaturesHash = "0x7a927ad3e689e4f3679fe1d1b8ea1088b914523b0c2da0d6dc0938e5e5cf8d15"
        let signatureProof = TonValidatorSignatureProofInput(
            totalWeight: 3,
            signedWeight: 3,
            blockMessageHash: blockMessageHash,
            validatorPublicKeys: validatorPublicKeys,
            validatorWeights: validatorWeights,
            signersBitmap: Data([0x03]),
            signatures: [Data(repeating: 0xab, count: 64), Data(repeating: 0xcd, count: 64)]
        )

        XCTAssertEqual(
            try canonicalTonMasterchainBlockMessageBytes(
                sourceDomain: sccpDomainTon,
                masterchainSeqno: 19,
                masterchainWorkchainId: -1,
                masterchainShard: 0x8000_0000_0000_0000,
                masterchainBlockHash: String(repeating: "aa", count: 32),
                masterchainFileHash: String(repeating: "a5", count: 32),
                validatorSetHash: validatorSetHash,
                masterchainConfigRoot: configRoot,
                masterchainConfigProofHash: configProofHash,
                shardWorkchainId: 0,
                shardShard: 0x8000_0000_0000_0000,
                shardSeqno: 7,
                shardBlockHash: String(repeating: "bb", count: 32),
                shardFileHash: String(repeating: "bc", count: 32),
                shardStateRoot: String(repeating: "cc", count: 32),
                transactionRoot: String(repeating: "dd", count: 32),
                shardProofHash: String(repeating: "ee", count: 32)
            ).count,
            365
        )
        XCTAssertEqual(
            try tonMasterchainBlockMessageHash(
                sourceDomain: sccpDomainTon,
                masterchainSeqno: 19,
                masterchainWorkchainId: -1,
                masterchainShard: 0x8000_0000_0000_0000,
                masterchainBlockHash: String(repeating: "aa", count: 32),
                masterchainFileHash: String(repeating: "a5", count: 32),
                validatorSetHash: validatorSetHash,
                masterchainConfigRoot: configRoot,
                masterchainConfigProofHash: configProofHash,
                shardWorkchainId: 0,
                shardShard: 0x8000_0000_0000_0000,
                shardSeqno: 7,
                shardBlockHash: String(repeating: "bb", count: 32),
                shardFileHash: String(repeating: "bc", count: 32),
                shardStateRoot: String(repeating: "cc", count: 32),
                transactionRoot: String(repeating: "dd", count: 32),
                shardProofHash: String(repeating: "ee", count: 32)
            ),
            blockMessageHash
        )
        XCTAssertThrowsError(try canonicalTonMasterchainBlockMessageBytes(
            sourceDomain: sccpDomainTon,
            masterchainSeqno: 19,
            masterchainWorkchainId: 0,
            masterchainShard: 0x8000_0000_0000_0000,
            masterchainBlockHash: String(repeating: "aa", count: 32),
            masterchainFileHash: String(repeating: "a5", count: 32),
            validatorSetHash: validatorSetHash,
            masterchainConfigRoot: configRoot,
            masterchainConfigProofHash: configProofHash,
            shardWorkchainId: 0,
            shardShard: 0x8000_0000_0000_0000,
            shardSeqno: 7,
            shardBlockHash: String(repeating: "bb", count: 32),
            shardFileHash: String(repeating: "bc", count: 32),
            shardStateRoot: String(repeating: "cc", count: 32),
            transactionRoot: String(repeating: "dd", count: 32),
            shardProofHash: String(repeating: "ee", count: 32)
        ))
        XCTAssertEqual(
            try canonicalTonMasterchainValidatorSignaturesBytes(
                signatureProof,
                validatorSetHash: validatorSetHash
            ).count,
            322
        )
        XCTAssertEqual(
            try tonMasterchainValidatorSignaturesHash(signatureProof, validatorSetHash: validatorSetHash),
            signaturesHash
        )
        XCTAssertThrowsError(try canonicalTonMasterchainValidatorSignaturesBytes(
            signatureProof,
            validatorSetHash: String(repeating: "bb", count: 32)
        ))
        XCTAssertThrowsError(try canonicalTonMasterchainValidatorSignaturesBytes(
            TonValidatorSignatureProofInput(
                totalWeight: 3,
                signedWeight: 3,
                blockMessageHash: blockMessageHash,
                validatorPublicKeys: validatorPublicKeys,
                validatorWeights: validatorWeights,
                signersBitmap: Data([0x03]),
                signatures: [Data(repeating: 0, count: 64), Data(repeating: 0x01, count: 64)]
            ),
            validatorSetHash: validatorSetHash
        ))
    }

    func testTonProverRequiresLinkedProofEngine() async throws {
        let prover = TonSccpProver()

        do {
            _ = try await prover.prove(Self.sampleTonProofRequestInput())
            XCTFail("expected localProverUnavailable")
        } catch let error as TonSccpProverError {
            XCTAssertEqual(error, .localProverUnavailable)
        }
    }

    func testTonProverRejectsNonProductionInputBeforeLinkedProofEngine() async throws {
        var invoked = false
        let prover = TonSccpProver { _ in
            invoked = true
            return Data([1, 2, 3, 4])
        }

        do {
            _ = try await prover.prove(Self.sampleTonProofRequestInput(
                sourceProofBytes: Data([9, 10]),
                sourceStateVerifierHash: sccpZeroHashV1
            ))
            XCTFail("expected invalidHex32")
        } catch let error as TonSccpProverError {
            XCTAssertEqual(error, .invalidHex32("sourceStateVerifierHash"))
        }
        XCTAssertFalse(invoked)
    }

    func testTonProverWrapsExternalProofBytes() async throws {
        var seenTonProofRequests: [TonSccpProofRequest] = []
        let canonicalTonBundleBytes = Self.sampleTonBundleBytes()
        let alternateTonBundleBytes = Self.sampleTonBundleBytes(finalityProof: Data([0x71, 0x73]))
        let prover = TonSccpProver { request in
            seenTonProofRequests.append(request)
            XCTAssertEqual(request.backend, sccpTonContractProofBackendV1)
            XCTAssertEqual(request.statementHash, "0x" + String(repeating: "56", count: 32))
            XCTAssertEqual(request.destinationBindingHash, "0x" + String(repeating: "78", count: 32))
            var callbackBundleBytes = request.bundleBytes
            callbackBundleBytes[callbackBundleBytes.startIndex] =
                callbackBundleBytes[callbackBundleBytes.startIndex] == 0 ? 1 : 0
            var callbackSourceProofBytes = request.sourceProofBytes
            if !callbackSourceProofBytes.isEmpty {
                callbackSourceProofBytes[callbackSourceProofBytes.startIndex] =
                    callbackSourceProofBytes[callbackSourceProofBytes.startIndex] == 0 ? 1 : 0
                XCTAssertEqual(request.sourceProofBytes, Data([9, 10]))
                XCTAssertNotEqual(callbackSourceProofBytes, request.sourceProofBytes)
            }
            XCTAssertEqual(request.bundleBytes, canonicalTonBundleBytes)
            return Data([1, 2, 3, 4])
        }

        let sourceProofInput = Self.sampleTonProofRequestInput(sourceProofBytes: Data([9, 10]))
        let omittedSourceInput = Self.sampleTonProofRequestInput()
        let result = try await prover.prove(sourceProofInput)
        let omittedSourceResult = try await prover.prove(omittedSourceInput)
        XCTAssertTrue(omittedSourceResult.sourceProofBytes.isEmpty)
        XCTAssertEqual(
            seenTonProofRequests,
            [
                try buildTonSccpProofRequest(sourceProofInput),
                try buildTonSccpProofRequest(omittedSourceInput),
            ]
        )

        XCTAssertEqual(result.proofBytes, Data([1, 2, 3, 4]))
        XCTAssertEqual(result.proofBase64, "AQIDBA==")
        XCTAssertEqual(result.statementHash, "0x" + String(repeating: "56", count: 32))
        XCTAssertEqual(result.destinationBindingHash, "0x" + String(repeating: "78", count: 32))
        XCTAssertEqual(result.sourceStateVerifierId, sccpTonMainnetShardStateVerifierIdV1)
        XCTAssertEqual(result.sourceStateVerifierHash, "0x" + String(repeating: "cc", count: 32))
        XCTAssertTrue(result.envelopeHash.hasPrefix("0x"))
        XCTAssertEqual(result.envelopeHash.count, 66)
        XCTAssertTrue(result.requestHash.hasPrefix("0x"))
        XCTAssertEqual(result.requestHash.count, 66)

        let submissionInput = try TonSccpMessageBodyInput(
            proofResult: result,
            bundleBytes: canonicalTonBundleBytes,
            metadataBytes: Data([8, 9]),
            queryId: 7
        )
        XCTAssertEqual(submissionInput.publicInputs, result.publicInputs)
        XCTAssertEqual(submissionInput.proofBytes, result.proofBytes)
        XCTAssertEqual(result.bundleBytes, canonicalTonBundleBytes)
        XCTAssertEqual(result.sourceProofBytes, Data([9, 10]))
        XCTAssertEqual(submissionInput.statementHash, result.proofContext.statementHash)
        XCTAssertEqual(submissionInput.destinationBindingHash, result.proofContext.destinationBindingHash)
        let submission = try buildTonSccpSubmission(submissionInput)
        XCTAssertEqual(submission.submissionKind, "internal_message")
        XCTAssertEqual(submission.verifierEntrypoint, "op::submit_sccp_message_proof")
        let oversizedTonMessageResult = try wrapTonSccpProofResult(
            proofBytes: Data(repeating: 1, count: 4096 * 127),
            request: try buildTonSccpProofRequest(Self.sampleTonProofRequestInput())
        )
        let oversizedTonMessageInput = try TonSccpMessageBodyInput(
            proofResult: oversizedTonMessageResult,
            bundleBytes: canonicalTonBundleBytes,
            metadataBytes: Data([8, 9])
        )
        XCTAssertThrowsError(try buildTonSccpSubmission(oversizedTonMessageInput)) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("messageBodyBoc"))
        }
        let omittedSourceWrapped = try wrapTonSccpProofResult(
            proofBytes: Data([1, 2, 3, 4]),
            request: try buildTonSccpProofRequest(Self.sampleTonProofRequestInput())
        )
        XCTAssertTrue(omittedSourceWrapped.sourceProofBytes.isEmpty)
        let omittedSourceSubmissionInput = try TonSccpMessageBodyInput(
            proofResult: omittedSourceResult,
            bundleBytes: canonicalTonBundleBytes
        )
        XCTAssertEqual(omittedSourceSubmissionInput.bundleBytes, canonicalTonBundleBytes)

        XCTAssertThrowsError(try TonSccpMessageBodyInput(
            proofResult: result,
            bundleBytes: alternateTonBundleBytes
        )) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("bundleBytes"))
        }

        let tamperedBundleResult = TonSccpProofResult(
            version: result.version,
            backend: result.backend,
            proofBytes: result.proofBytes,
            proofBase64: result.proofBase64,
            publicInputs: result.publicInputs,
            bundleBytes: alternateTonBundleBytes,
            sourceProofBytes: result.sourceProofBytes,
            proofContext: result.proofContext,
            statementHash: result.statementHash,
            destinationBindingHash: result.destinationBindingHash,
            sourceStateVerifierId: result.sourceStateVerifierId,
            sourceStateVerifierHash: result.sourceStateVerifierHash,
            sourceAdapterDeploymentBindingHash: result.sourceAdapterDeploymentBindingHash,
            sourceAdapterDeploymentBinding: result.sourceAdapterDeploymentBinding,
            requestHash: result.requestHash,
            envelopeHash: result.envelopeHash
        )
        XCTAssertThrowsError(try TonSccpMessageBodyInput(
            proofResult: tamperedBundleResult,
            bundleBytes: alternateTonBundleBytes
        )) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("proofResult.requestHash"))
        }

        let mismatchedProofBase64Result = TonSccpProofResult(
            version: result.version,
            backend: result.backend,
            proofBytes: result.proofBytes,
            proofBase64: "AAAA",
            publicInputs: result.publicInputs,
            bundleBytes: result.bundleBytes,
            sourceProofBytes: result.sourceProofBytes,
            proofContext: result.proofContext,
            statementHash: result.statementHash,
            destinationBindingHash: result.destinationBindingHash,
            sourceStateVerifierId: result.sourceStateVerifierId,
            sourceStateVerifierHash: result.sourceStateVerifierHash,
            sourceAdapterDeploymentBindingHash: result.sourceAdapterDeploymentBindingHash,
            sourceAdapterDeploymentBinding: result.sourceAdapterDeploymentBinding,
            requestHash: result.requestHash,
            envelopeHash: result.envelopeHash
        )
        XCTAssertThrowsError(try TonSccpMessageBodyInput(
            proofResult: mismatchedProofBase64Result,
            bundleBytes: canonicalTonBundleBytes
        )) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("proofResult.proofBase64"))
        }

        let missingEnvelopeResult = TonSccpProofResult(
            version: result.version,
            backend: result.backend,
            proofBytes: result.proofBytes,
            proofBase64: result.proofBase64,
            publicInputs: result.publicInputs,
            proofContext: result.proofContext,
            statementHash: result.statementHash,
            destinationBindingHash: result.destinationBindingHash,
            sourceStateVerifierId: result.sourceStateVerifierId,
            sourceStateVerifierHash: result.sourceStateVerifierHash,
            sourceAdapterDeploymentBindingHash: result.sourceAdapterDeploymentBindingHash,
            sourceAdapterDeploymentBinding: result.sourceAdapterDeploymentBinding,
            requestHash: result.requestHash,
            envelopeHash: sccpZeroHashV1
        )
        XCTAssertThrowsError(try TonSccpMessageBodyInput(
            proofResult: missingEnvelopeResult,
            bundleBytes: canonicalTonBundleBytes
        )) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidHex32("proofResult.envelopeHash"))
        }

        let tamperedEnvelopeResult = TonSccpProofResult(
            version: result.version,
            backend: result.backend,
            proofBytes: result.proofBytes,
            proofBase64: result.proofBase64,
            publicInputs: result.publicInputs,
            proofContext: result.proofContext,
            statementHash: result.statementHash,
            destinationBindingHash: result.destinationBindingHash,
            sourceStateVerifierId: result.sourceStateVerifierId,
            sourceStateVerifierHash: result.sourceStateVerifierHash,
            sourceAdapterDeploymentBindingHash: result.sourceAdapterDeploymentBindingHash,
            sourceAdapterDeploymentBinding: result.sourceAdapterDeploymentBinding,
            requestHash: result.requestHash,
            envelopeHash: "0x" + String(repeating: "aa", count: 32)
        )
        XCTAssertThrowsError(try TonSccpMessageBodyInput(
            proofResult: tamperedEnvelopeResult,
            bundleBytes: canonicalTonBundleBytes
        )) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("proofResult.envelopeHash"))
        }

        let mismatchedProofContextResult = TonSccpProofResult(
            version: result.version,
            backend: result.backend,
            proofBytes: result.proofBytes,
            proofBase64: result.proofBase64,
            publicInputs: result.publicInputs,
            proofContext: TonSccpProofContext(
                version: result.proofContext.version,
                statementHash: "0x" + String(repeating: "99", count: 32),
                destinationBindingHash: result.proofContext.destinationBindingHash
            ),
            statementHash: result.statementHash,
            destinationBindingHash: result.destinationBindingHash,
            sourceStateVerifierId: result.sourceStateVerifierId,
            sourceStateVerifierHash: result.sourceStateVerifierHash,
            sourceAdapterDeploymentBindingHash: result.sourceAdapterDeploymentBindingHash,
            sourceAdapterDeploymentBinding: result.sourceAdapterDeploymentBinding,
            requestHash: result.requestHash,
            envelopeHash: result.envelopeHash
        )
        XCTAssertThrowsError(try TonSccpMessageBodyInput(
            proofResult: mismatchedProofContextResult,
            bundleBytes: canonicalTonBundleBytes
        )) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("proofResult.proofContext"))
        }

        let wrongSourceStateVerifierResult = TonSccpProofResult(
            version: result.version,
            backend: result.backend,
            proofBytes: result.proofBytes,
            proofBase64: result.proofBase64,
            publicInputs: result.publicInputs,
            proofContext: result.proofContext,
            statementHash: result.statementHash,
            destinationBindingHash: result.destinationBindingHash,
            sourceStateVerifierId: result.sourceStateVerifierId,
            sourceStateVerifierHash: sccpZeroHashV1,
            sourceAdapterDeploymentBindingHash: result.sourceAdapterDeploymentBindingHash,
            sourceAdapterDeploymentBinding: result.sourceAdapterDeploymentBinding,
            requestHash: result.requestHash,
            envelopeHash: result.envelopeHash
        )
        XCTAssertThrowsError(try TonSccpMessageBodyInput(
            proofResult: wrongSourceStateVerifierResult,
            bundleBytes: canonicalTonBundleBytes
        )) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidHex32("proofResult.sourceStateVerifierHash"))
        }

        let wrongDeploymentBindingResult = TonSccpProofResult(
            version: result.version,
            backend: result.backend,
            proofBytes: result.proofBytes,
            proofBase64: result.proofBase64,
            publicInputs: result.publicInputs,
            proofContext: result.proofContext,
            statementHash: result.statementHash,
            destinationBindingHash: result.destinationBindingHash,
            sourceStateVerifierId: result.sourceStateVerifierId,
            sourceStateVerifierHash: result.sourceStateVerifierHash,
            sourceAdapterDeploymentBindingHash: result.sourceAdapterDeploymentBindingHash,
            sourceAdapterDeploymentBinding: TonSccpSourceAdapterDeploymentBinding(
                version: result.sourceAdapterDeploymentBinding.version,
                sourceDomain: result.sourceAdapterDeploymentBinding.sourceDomain,
                targetDomain: sccpDomainTon,
                sourceAdapterDeploymentHash: result.sourceAdapterDeploymentBinding.sourceAdapterDeploymentHash,
                sourceAdapterDeploymentReceiptHash: result.sourceAdapterDeploymentBinding.sourceAdapterDeploymentReceiptHash
            ),
            requestHash: result.requestHash,
            envelopeHash: result.envelopeHash
        )
        XCTAssertThrowsError(try TonSccpMessageBodyInput(
            proofResult: wrongDeploymentBindingResult,
            bundleBytes: canonicalTonBundleBytes
        )) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("proofResult.sourceAdapterDeploymentBinding.targetDomain"))
        }

        let request = try buildTonSccpProofRequest(Self.sampleTonProofRequestInput(sourceProofBytes: Data([9, 10])))
        let mismatchedRequest = TonSccpProofRequest(
            version: request.version,
            backend: request.backend,
            sourceDomain: request.sourceDomain,
            targetDomain: request.targetDomain,
            publicInputs: request.publicInputs,
            publicInputsBytes: request.publicInputsBytes,
            bundleBytes: request.bundleBytes,
            sourceProofBytes: request.sourceProofBytes,
            proofContext: request.proofContext,
            statementHash: request.statementHash,
            destinationBindingHash: request.destinationBindingHash,
            sourceStateVerifierId: request.sourceStateVerifierId,
            sourceStateVerifierHash: request.sourceStateVerifierHash,
            sourceAdapterDeploymentBindingHash: "0x" + String(repeating: "cc", count: 32),
            sourceAdapterDeploymentBinding: request.sourceAdapterDeploymentBinding,
            requestHash: request.requestHash
        )
        XCTAssertThrowsError(try wrapTonSccpProofResult(
            proofBytes: Data([1, 2, 3, 4]),
            request: mismatchedRequest
        )) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("request"))
        }
        XCTAssertThrowsError(try wrapTonSccpProofResult(
            proofBytes: Data([0, 0]),
            request: request
        )) { error in
            XCTAssertEqual(error as? TonSccpProverError, .allZeroProof)
        }
        XCTAssertThrowsError(try wrapTonSccpProofResult(
            proofBytes: Data(repeating: 1, count: sccpNativeRecursiveMaxProofBytes + 1),
            request: request
        )) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("proofBytes"))
        }
    }

    func testTonProverResolvesWitnessProviderBeforeBuildingRequest() async throws {
        let witnessProvider = TonSourceProofWitnessProvider()
        let prover = TonSccpProver(
            witnessProvider: witnessProvider,
            proveFunction: { request in
                XCTAssertEqual(witnessProvider.resolveCount, 1)
                XCTAssertEqual(request.sourceProofBytes, Data([9, 10]))
                return Data([1, 2, 3, 4])
            }
        )

        let result = try await prover.prove(Self.sampleTonProofRequestInput())

        XCTAssertEqual(witnessProvider.resolveCount, 1)
        XCTAssertEqual(result.sourceProofBytes, Data([9, 10]))
    }

    func testTonProofRequestBindsRelayContextAndSourceAdapterDeployment() throws {
        let request = try buildTonSccpProofRequest(Self.sampleTonProofRequestInput(
            sourceAdapterDeploymentHash: String(repeating: "aa", count: 32),
            sourceAdapterDeploymentReceiptHash: String(repeating: "bb", count: 32)
        ))

        XCTAssertEqual(request.proofContext.statementHash, "0x" + String(repeating: "56", count: 32))
        XCTAssertEqual(request.proofContext.destinationBindingHash, "0x" + String(repeating: "78", count: 32))
        XCTAssertEqual(request.sourceStateVerifierId, sccpTonMainnetShardStateVerifierIdV1)
        XCTAssertEqual(request.sourceStateVerifierHash, "0x" + String(repeating: "cc", count: 32))
        XCTAssertEqual(request.sourceAdapterDeploymentBinding.sourceDomain, sccpDomainTon)
        XCTAssertEqual(request.sourceAdapterDeploymentBinding.targetDomain, sccpDomainSora)
        XCTAssertEqual(request.sourceAdapterDeploymentBinding.sourceAdapterDeploymentHash, "0x" + String(repeating: "aa", count: 32))
        XCTAssertEqual(request.sourceAdapterDeploymentBinding.sourceAdapterDeploymentReceiptHash, "0x" + String(repeating: "bb", count: 32))
        XCTAssertEqual(
            request.sourceAdapterDeploymentBindingHash,
            try sccpSourceAdapterDeploymentBindingHash(request.sourceAdapterDeploymentBinding)
        )
        XCTAssertTrue(request.requestHash.hasPrefix("0x"))
        XCTAssertEqual(request.requestHash.count, 66)
        let deploymentBinding = TonSccpSourceAdapterDeploymentBinding(
            version: 1,
            sourceDomain: sccpDomainTon,
            targetDomain: sccpDomainSora,
            sourceAdapterDeploymentHash: String(repeating: "aa", count: 32),
            sourceAdapterDeploymentReceiptHash: String(repeating: "bb", count: 32)
        )
        let bindingRequest = try buildTonSccpProofRequest(TonSccpProofRequestInput(
            publicInputs: Self.sampleTonPublicInputs(),
            bundleBytes: Self.sampleTonBundleBytes(),
            statementHash: String(repeating: "56", count: 32),
            destinationBindingHash: String(repeating: "78", count: 32),
            sourceStateVerifierHash: String(repeating: "cc", count: 32),
            sourceAdapterDeploymentBinding: deploymentBinding
        ))
        XCTAssertEqual(bindingRequest.requestHash, request.requestHash)
        XCTAssertThrowsError(try TonSccpProofRequestInput(
            publicInputs: Self.sampleTonPublicInputs(),
            bundleBytes: Self.sampleTonBundleBytes(),
            statementHash: String(repeating: "56", count: 32),
            destinationBindingHash: String(repeating: "78", count: 32),
            sourceStateVerifierHash: String(repeating: "cc", count: 32),
            sourceAdapterDeploymentBinding: TonSccpSourceAdapterDeploymentBinding(
                version: 1,
                sourceDomain: sccpDomainTon,
                targetDomain: sccpDomainTon,
                sourceAdapterDeploymentHash: String(repeating: "aa", count: 32),
                sourceAdapterDeploymentReceiptHash: String(repeating: "bb", count: 32)
            )
        )) { error in
            XCTAssertEqual(
                error as? TonSccpProverError,
                .invalidField("sourceAdapterDeploymentBinding.targetDomain")
            )
        }
        let sourceStateBoundRequest = try buildTonSccpProofRequest(Self.sampleTonProofRequestInput(
            sourceStateVerifierHash: String(repeating: "dd", count: 32),
            sourceAdapterDeploymentHash: String(repeating: "aa", count: 32),
            sourceAdapterDeploymentReceiptHash: String(repeating: "bb", count: 32)
        ))
        XCTAssertNotEqual(sourceStateBoundRequest.requestHash, request.requestHash)
        let splitBoundaryRequest = try buildTonSccpProofRequest(Self.sampleTonProofRequestInput(
            bundleBytes: Self.sampleTonBundleBytes(),
            sourceProofBytes: Data([9, 10]),
            sourceAdapterDeploymentHash: String(repeating: "aa", count: 32),
            sourceAdapterDeploymentReceiptHash: String(repeating: "bb", count: 32)
        ))
        let shiftedSplitRequest = try buildTonSccpProofRequest(Self.sampleTonProofRequestInput(
            bundleBytes: Self.sampleTonBundleBytes(finalityProof: Data([0x71, 0x73])),
            sourceProofBytes: Data([10]),
            sourceAdapterDeploymentHash: String(repeating: "aa", count: 32),
            sourceAdapterDeploymentReceiptHash: String(repeating: "bb", count: 32)
        ))
        XCTAssertNotEqual(splitBoundaryRequest.requestHash, shiftedSplitRequest.requestHash)
        XCTAssertThrowsError(try buildTonSccpProofRequest(Self.sampleTonProofRequestInput(
            sourceStateVerifierId: "debug-ton-state-verifier",
            sourceStateVerifierHash: String(repeating: "cc", count: 32)
        ))) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("sourceStateVerifierId"))
        }
        XCTAssertThrowsError(try buildTonSccpProofRequest(Self.sampleTonProofRequestInput(
            sourceStateVerifierHash: sccpZeroHashV1
        ))) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidHex32("sourceStateVerifierHash"))
        }
        XCTAssertThrowsError(try buildTonSccpProofRequest(Self.sampleTonProofRequestInput(
            publicInputs: Self.sampleTonPublicInputs(payloadHash: " " + String(repeating: "ee", count: 32))
        ))) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidHex32("payloadHash"))
        }
        XCTAssertThrowsError(try buildTonSccpProofRequest(Self.sampleTonProofRequestInput(
            statementHash: String(repeating: "56", count: 32) + "\n"
        ))) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidHex32("statementHash"))
        }
        XCTAssertThrowsError(try buildTonSccpProofRequest(Self.sampleTonProofRequestInput(
            statementHash: String(repeating: "00", count: 32)
        ))) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidHex32("statementHash"))
        }
        XCTAssertThrowsError(try buildTonSccpProofRequest(Self.sampleTonProofRequestInput(
            destinationBindingHash: String(repeating: "00", count: 32)
        ))) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidHex32("destinationBindingHash"))
        }
        XCTAssertThrowsError(try buildTonSccpProofRequest(Self.sampleTonProofRequestInput(
            sourceAdapterDeploymentHash: "\n" + String(repeating: "aa", count: 32),
            sourceAdapterDeploymentReceiptHash: String(repeating: "bb", count: 32)
        ))) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidHex32("sourceAdapterDeploymentHash"))
        }
        XCTAssertThrowsError(try buildTonSccpProofRequest(Self.sampleTonProofRequestInput(
            sourceAdapterDeploymentHash: String(repeating: "aa", count: 32),
            sourceAdapterDeploymentReceiptHash: sccpZeroHashV1
        ))) { error in
            XCTAssertEqual(error as? TonSccpProverError, .sourceAdapterDeploymentBindingMismatch)
        }
        XCTAssertThrowsError(try buildTonSccpProofRequest(Self.sampleTonProofRequestInput(
            sourceAdapterDeploymentHash: sccpZeroHashV1,
            sourceAdapterDeploymentReceiptHash: sccpZeroHashV1
        ))) { error in
            XCTAssertEqual(error as? TonSccpProverError, .sourceAdapterDeploymentBindingMismatch)
        }
        XCTAssertThrowsError(try buildTonSccpProofRequest(Self.sampleTonProofRequestInput(
            bundleBytes: Data(),
            sourceAdapterDeploymentHash: String(repeating: "aa", count: 32),
            sourceAdapterDeploymentReceiptHash: String(repeating: "bb", count: 32)
        ))) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("bundleBytes"))
        }
        XCTAssertThrowsError(try buildTonSccpProofRequest(Self.sampleTonProofRequestInput(
            bundleBytes: Data([0, 0]),
            sourceAdapterDeploymentHash: String(repeating: "aa", count: 32),
            sourceAdapterDeploymentReceiptHash: String(repeating: "bb", count: 32)
        ))) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("bundleBytes"))
        }
        XCTAssertThrowsError(try buildTonSccpProofRequest(Self.sampleTonProofRequestInput(
            bundleBytes: Data(repeating: 1, count: sccpNativeRecursiveMaxProofBytes + 1),
            sourceAdapterDeploymentHash: String(repeating: "aa", count: 32),
            sourceAdapterDeploymentReceiptHash: String(repeating: "bb", count: 32)
        ))) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("bundleBytes"))
        }
        XCTAssertThrowsError(try buildTonSccpProofRequest(Self.sampleTonProofRequestInput(
            sourceProofBytes: Data(repeating: 1, count: sccpSourceStateMaxProofBytes + 1),
            sourceAdapterDeploymentHash: String(repeating: "aa", count: 32),
            sourceAdapterDeploymentReceiptHash: String(repeating: "bb", count: 32)
        ))) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("sourceProofBytes"))
        }
        XCTAssertThrowsError(try buildTonSccpProofRequest(Self.sampleTonProofRequestInput(
            sourceDomain: sccpDomainSolana
        ))) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidSourceDomain(sccpDomainSolana))
        }
        XCTAssertThrowsError(try buildTonSccpProofRequest(TonSccpProofRequestInput(
            publicInputs: Self.sampleTonPublicInputs(targetDomain: sccpDomainSolana),
            bundleBytes: Self.sampleTonBundleBytes(),
            statementHash: String(repeating: "56", count: 32),
            destinationBindingHash: String(repeating: "78", count: 32),
            sourceStateVerifierHash: String(repeating: "cc", count: 32),
            sourceAdapterDeploymentHash: String(repeating: "aa", count: 32),
            sourceAdapterDeploymentReceiptHash: String(repeating: "bb", count: 32)
        ))) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("publicInputs.targetDomain"))
        }
        XCTAssertThrowsError(try buildTonSccpProofRequest(Self.sampleTonProofRequestInput(
            backend: "debug-ton-backend"
        ))) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidBranch("backend"))
        }
    }

    func testTonProofRequestRejectsNoncanonicalOrMismatchedBundleBytes() throws {
        let base = Self.sampleTonProofRequestInput(
            sourceAdapterDeploymentHash: String(repeating: "aa", count: 32),
            sourceAdapterDeploymentReceiptHash: String(repeating: "bb", count: 32)
        )

        XCTAssertThrowsError(try buildTonSccpProofRequest(Self.sampleTonProofRequestInput(
            bundleBytes: Data([5, 6, 7]),
            sourceAdapterDeploymentHash: String(repeating: "aa", count: 32),
            sourceAdapterDeploymentReceiptHash: String(repeating: "bb", count: 32)
        ))) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("bundleBytes.version"))
        }

        let swapped = Self.sampleTonBundleFixture(amount: 43)
        XCTAssertThrowsError(try buildTonSccpProofRequest(Self.sampleTonProofRequestInput(
            publicInputs: base.publicInputs,
            bundleBytes: swapped.bundleBytes,
            sourceAdapterDeploymentHash: String(repeating: "aa", count: 32),
            sourceAdapterDeploymentReceiptHash: String(repeating: "bb", count: 32)
        ))) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("bundleBytes"))
        }

        var tamperedCommitment = Self.sampleTonBundleBytes()
        tamperedCommitment[37 + 69] ^= 0x01
        XCTAssertThrowsError(try buildTonSccpProofRequest(Self.sampleTonProofRequestInput(
            bundleBytes: tamperedCommitment,
            sourceAdapterDeploymentHash: String(repeating: "aa", count: 32),
            sourceAdapterDeploymentReceiptHash: String(repeating: "bb", count: 32)
        ))) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("bundleBytes.commitment"))
        }

        var tamperedRoot = Self.sampleTonBundleBytes()
        tamperedRoot[1] ^= 0x01
        XCTAssertThrowsError(try buildTonSccpProofRequest(Self.sampleTonProofRequestInput(
            bundleBytes: tamperedRoot,
            sourceAdapterDeploymentHash: String(repeating: "aa", count: 32),
            sourceAdapterDeploymentReceiptHash: String(repeating: "bb", count: 32)
        ))) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("bundleBytes.commitment_root"))
        }

        let ranges = Self.splitTestTonSccpMessageProofBundleBytes(Self.sampleTonBundleBytes())
        let payloadRange = ranges["payload"]!
        var payloadWithTrailingByte = payloadRange.bytes
        payloadWithTrailingByte.append(0)
        XCTAssertThrowsError(try buildTonSccpProofRequest(Self.sampleTonProofRequestInput(
            bundleBytes: Self.replaceTestTonSccpMessageProofBundleVec(
                Self.sampleTonBundleBytes(),
                range: payloadRange,
                replacement: payloadWithTrailingByte
            ),
            sourceAdapterDeploymentHash: String(repeating: "aa", count: 32),
            sourceAdapterDeploymentReceiptHash: String(repeating: "bb", count: 32)
        ))) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("bundleBytes.payload"))
        }

        var unsupportedPayloadKind = payloadRange.bytes
        unsupportedPayloadKind[unsupportedPayloadKind.startIndex] = 0xff
        XCTAssertThrowsError(try buildTonSccpProofRequest(Self.sampleTonProofRequestInput(
            bundleBytes: Self.replaceTestTonSccpMessageProofBundleVec(
                Self.sampleTonBundleBytes(),
                range: payloadRange,
                replacement: unsupportedPayloadKind
            ),
            sourceAdapterDeploymentHash: String(repeating: "aa", count: 32),
            sourceAdapterDeploymentReceiptHash: String(repeating: "bb", count: 32)
        ))) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("bundleBytes.payload"))
        }

        let merkleProofRange = ranges["merkleProof"]!
        var merkleProofWithTrailingByte = merkleProofRange.bytes
        merkleProofWithTrailingByte.append(0)
        XCTAssertThrowsError(try buildTonSccpProofRequest(Self.sampleTonProofRequestInput(
            bundleBytes: Self.replaceTestTonSccpMessageProofBundleVec(
                Self.sampleTonBundleBytes(),
                range: merkleProofRange,
                replacement: merkleProofWithTrailingByte
            ),
            sourceAdapterDeploymentHash: String(repeating: "aa", count: 32),
            sourceAdapterDeploymentReceiptHash: String(repeating: "bb", count: 32)
        ))) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("bundleBytes.merkle_proof"))
        }

        let oneStep = Self.sampleTonBundleFixture(
            merkleProofSteps: [(Data(repeating: 0xcc, count: 32), 1)]
        )
        let oneStepRanges = Self.splitTestTonSccpMessageProofBundleBytes(oneStep.bundleBytes)
        let oneStepMerkleProofRange = oneStepRanges["merkleProof"]!
        var invalidDirection = oneStepMerkleProofRange.bytes
        invalidDirection[4 + 32] = 2
        XCTAssertThrowsError(try buildTonSccpProofRequest(Self.sampleTonProofRequestInput(
            publicInputs: oneStep.publicInputs,
            bundleBytes: Self.replaceTestTonSccpMessageProofBundleVec(
                oneStep.bundleBytes,
                range: oneStepMerkleProofRange,
                replacement: invalidDirection
            ),
            sourceAdapterDeploymentHash: String(repeating: "aa", count: 32),
            sourceAdapterDeploymentReceiptHash: String(repeating: "bb", count: 32)
        ))) { error in
            XCTAssertEqual(
                error as? TonSccpProverError,
                .invalidField("bundleBytes.merkle_proof.steps[0].sibling_is_left")
            )
        }

        let nonSora = Self.sampleTonBundleFixture(
            sourceDomain: sccpDomainSolana,
            senderCodec: 3,
            sender: "11111111111111111111111111111111"
        )
        XCTAssertThrowsError(try buildTonSccpProofRequest(Self.sampleTonProofRequestInput(
            publicInputs: nonSora.publicInputs,
            bundleBytes: nonSora.bundleBytes,
            sourceProofBytes: Data(),
            sourceAdapterDeploymentHash: String(repeating: "aa", count: 32),
            sourceAdapterDeploymentReceiptHash: String(repeating: "bb", count: 32)
        ))) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("sourceProofBytes"))
        }

        let nonSoraRequest = try buildTonSccpProofRequest(Self.sampleTonProofRequestInput(
            publicInputs: nonSora.publicInputs,
            bundleBytes: nonSora.bundleBytes,
            sourceProofBytes: Data([9, 10]),
            sourceAdapterDeploymentHash: String(repeating: "aa", count: 32),
            sourceAdapterDeploymentReceiptHash: String(repeating: "bb", count: 32)
        ))
        let nonSoraResult = try wrapTonSccpProofResult(
            proofBytes: Data([1, 2, 3, 4]),
            request: nonSoraRequest
        )
        let strippedSourceProofResult = TonSccpProofResult(
            version: nonSoraResult.version,
            backend: nonSoraResult.backend,
            proofBytes: nonSoraResult.proofBytes,
            proofBase64: nonSoraResult.proofBase64,
            publicInputs: nonSoraResult.publicInputs,
            bundleBytes: nonSoraResult.bundleBytes,
            sourceProofBytes: Data(),
            proofContext: nonSoraResult.proofContext,
            statementHash: nonSoraResult.statementHash,
            destinationBindingHash: nonSoraResult.destinationBindingHash,
            sourceStateVerifierId: nonSoraResult.sourceStateVerifierId,
            sourceStateVerifierHash: nonSoraResult.sourceStateVerifierHash,
            sourceAdapterDeploymentBindingHash: nonSoraResult.sourceAdapterDeploymentBindingHash,
            sourceAdapterDeploymentBinding: nonSoraResult.sourceAdapterDeploymentBinding,
            requestHash: nonSoraResult.requestHash,
            envelopeHash: nonSoraResult.envelopeHash
        )
        XCTAssertThrowsError(try TonSccpMessageBodyInput(
            proofResult: strippedSourceProofResult,
            bundleBytes: nonSora.bundleBytes
        )) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("sourceProofBytes"))
        }
    }

    func testTonProofRequestHashMatchesCrossSdkVector() throws {
        let publicInputs = Self.sampleTonPublicInputs()
        let request = try buildTonSccpProofRequest(TonSccpProofRequestInput(
            publicInputs: publicInputs,
            bundleBytes: Self.sampleTonBundleBytes(),
            sourceProofBytes: Data([0x51, 0x52, 0x53]),
            statementHash: String(repeating: "55", count: 32),
            destinationBindingHash: String(repeating: "66", count: 32),
            sourceStateVerifierHash: String(repeating: "42", count: 32),
            sourceAdapterDeploymentBinding: TonSccpSourceAdapterDeploymentBinding(
                version: 1,
                sourceDomain: sccpDomainTon,
                targetDomain: sccpDomainSora,
                sourceAdapterDeploymentHash: String(repeating: "aa", count: 32),
                sourceAdapterDeploymentReceiptHash: String(repeating: "bb", count: 32)
            )
        ))
        let expectedPublicInputsBytes = Data(hexString:
            "01" +
            "806384e356636c10ee3bbbb90674a80410a86be034616abb811586b21ac81fc4367a4f" +
            "9061f46a282eeeda95bc68c727888bde665bd89d0ebbc6dae266e3a264" +
            "04000000" +
            "377eb92928595d90759d66529f96acf34afd4ef64cd2327ab6f65876fb3cf93e" +
            "1300000000000000" +
            String(repeating: "aa", count: 32)
        )!

        XCTAssertEqual(try canonicalTonSccpPublicInputsBytes(publicInputs), expectedPublicInputsBytes)
        XCTAssertEqual(
            request.sourceAdapterDeploymentBindingHash,
            "0x7d35b186e3d49aed31693e33d33355fa8fa9032160c929f2c7fe260094f6ccdf"
        )
        XCTAssertEqual(
            request.requestHash,
            "0x2a292741b8e8d8454699eda954592904e8260e6b8a41cc840f5d9c48732c3bbe"
        )
        let proofResult = try wrapTonSccpProofResult(
            proofBytes: Data([0x91, 0x92, 0x93, 0x94, 0x95]),
            request: request
        )
        XCTAssertEqual(
            proofResult.envelopeHash,
            "0x9ed8e54d81c13a61939dedffb36c487f33d32a128ba95a0d29b33c5d25be6489"
        )
    }

    func testDerivesGroth16PublicSignalWordsForTron() throws {
        let signals = try sccpGroth16Bn254PublicSignalWords(
            publicInputs: Self.sampleTronPublicInputs(),
            sourceDomain: sccpDomainSora,
            statementHash: String(repeating: "55", count: 32),
            destinationBindingHash: String(repeating: "66", count: 32)
        )

        XCTAssertEqual(signals, [
            "0x0ffdbc782e79d1dc508e08af01e87f16d93b6e58e4861a0b8155455e3ee7a683",
            "0x0c5398ea95021a790e276e3ece1592b32b85751dc77e50293c867a5f2e0131bb",
            "0x21aac4195d8db839756f61c0780675823e15456c92acf135c36e02367c8fd11f",
            "0x01c73f2f9156a52493a9beabeec73e62deed32fcef2e3e6fac86a79f0764f0bc",
            "0x0ca6bbc36d23183d027c8df09f06c39e64abbb0bb4d6a4c37369d2c36f41a888",
            "0x2b153d0fe1bc6e2a6d44e851523edb1511dac55443ca80c22cbe9cb7423886dc",
            "0x2697e4e42f34b673b4aa254c6a92de09304e84c1a667c7d266777775a231efb4",
            "0x16fbe0c1d659f142b3e7815b24df66da3cfd89cc42d051b04bc31aae6925c396",
            "0x1157cd422e2089145c9cf93794dd6a0a1c3b1a611c22a5fe999d0542f62535d8",
        ])

        let changed = try sccpGroth16Bn254PublicSignalWords(
            publicInputs: Self.sampleTronPublicInputs(),
            sourceDomain: sccpDomainSora,
            statementHash: String(repeating: "55", count: 32),
            destinationBindingHash: String(repeating: "67", count: 32)
        )
        XCTAssertEqual(Array(signals.prefix(8)), Array(changed.prefix(8)))
        XCTAssertNotEqual(signals[8], changed[8])
    }

    func testTronProofRequestBindsPublicSignalsAndRelayContext() throws {
        let request = try buildTronSccpProofRequest(Self.sampleTronProofRequestInput(sourceProofBytes: Data([9, 10])))
        let expectedSignals = try sccpGroth16Bn254PublicSignalWords(
            publicInputs: Self.sampleTronPublicInputs(),
            sourceDomain: sccpDomainSora,
            statementHash: String(repeating: "56", count: 32),
            destinationBindingHash: String(repeating: "78", count: 32)
        )

        XCTAssertEqual(request.backend, sccpTronGroth16Bn254ProofBackendV1)
        XCTAssertEqual(request.sourceDomain, sccpDomainSora)
        XCTAssertEqual(request.targetDomain, sccpDomainTron)
        XCTAssertEqual(request.publicSignalWords, expectedSignals)
        XCTAssertEqual(request.proofContext.statementHash, "0x" + String(repeating: "56", count: 32))
        XCTAssertEqual(request.proofContext.destinationBindingHash, "0x" + String(repeating: "78", count: 32))
        XCTAssertTrue(request.requestHash.hasPrefix("0x"))
        XCTAssertEqual(request.requestHash.count, 66)

        let destinationBinding = try sccpTronDestinationBinding(
            networkId: "0x" + String(repeating: "33", count: 32),
            verifierAddress: "TJRabPrwbZy45sbavfcjinPJC18kjpRTv8",
            verifierCodeHash: "0x" + String(repeating: "bb", count: 32),
            verifierKeyHash: "0x" + String(repeating: "cc", count: 32)
        )
        let boundRequest = try buildTronSccpProofRequest(try TronSccpProofRequestInput(
            publicInputs: Self.sampleTronPublicInputs(),
            bundleBytes: Data([5, 6, 7]),
            sourceProofBytes: Data([9, 10]),
            statementHash: String(repeating: "56", count: 32),
            destinationBinding: destinationBinding
        ))
        XCTAssertEqual(boundRequest.destinationBindingHash, destinationBinding.hash)
        XCTAssertEqual(boundRequest.destinationBinding, destinationBinding)
        XCTAssertNotEqual(boundRequest.requestHash, request.requestHash)

        let forgedHashBinding = TronSccpDestinationBinding(
            version: destinationBinding.version,
            sourceDomain: destinationBinding.sourceDomain,
            targetDomain: destinationBinding.targetDomain,
            networkId: destinationBinding.networkId,
            verifierAddress: destinationBinding.verifierAddress,
            verifierCodeHash: destinationBinding.verifierCodeHash,
            verifierKeyHash: destinationBinding.verifierKeyHash,
            verifierBackend: destinationBinding.verifierBackend,
            proofFamily: destinationBinding.proofFamily,
            key: destinationBinding.key,
            hash: "0x" + String(repeating: "a7", count: 32)
        )
        XCTAssertThrowsError(try TronSccpProofRequestInput(
            publicInputs: Self.sampleTronPublicInputs(),
            bundleBytes: Data([5, 6, 7]),
            statementHash: String(repeating: "56", count: 32),
            destinationBinding: forgedHashBinding
        )) { error in
            XCTAssertEqual(error as? TronSccpProverError, .invalidPublicInputs("destinationBinding"))
        }

        let changed = try buildTronSccpProofRequest(Self.sampleTronProofRequestInput(
            sourceProofBytes: Data([9, 11])
        ))
        XCTAssertNotEqual(request.requestHash, changed.requestHash)
        let shiftedSplit = try buildTronSccpProofRequest(Self.sampleTronProofRequestInput(
            bundleBytes: Data([5, 6, 7, 9]),
            sourceProofBytes: Data([10])
        ))
        XCTAssertNotEqual(request.requestHash, shiftedSplit.requestHash)
        XCTAssertThrowsError(try buildTronSccpProofRequest(Self.sampleTronProofRequestInput(statementHash: ""))) { error in
            XCTAssertEqual(error as? TronSccpProverError, .invalidHex32("statementHash"))
        }
        XCTAssertThrowsError(try buildTronSccpProofRequest(Self.sampleTronProofRequestInput(
            publicInputs: Self.sampleTronPublicInputs(payloadHash: String(repeating: "00", count: 32))
        ))) { error in
            XCTAssertEqual(error as? TronSccpProverError, .zeroField("payloadHash"))
        }
        XCTAssertThrowsError(try buildTronSccpProofRequest(Self.sampleTronProofRequestInput(
            publicInputs: Self.sampleTronPublicInputs(payloadHash: " " + String(repeating: "22", count: 32))
        ))) { error in
            XCTAssertEqual(error as? TronSccpProverError, .invalidHex32("payloadHash"))
        }
        XCTAssertThrowsError(try buildTronSccpProofRequest(Self.sampleTronProofRequestInput(
            statementHash: String(repeating: "56", count: 32) + "\n"
        ))) { error in
            XCTAssertEqual(error as? TronSccpProverError, .invalidHex32("statementHash"))
        }
        XCTAssertThrowsError(try buildTronSccpProofRequest(Self.sampleTronProofRequestInput(
            bundleBytes: Data()
        ))) { error in
            XCTAssertEqual(error as? TronSccpProverError, .invalidPublicInputs("bundleBytes"))
        }
        XCTAssertThrowsError(try buildTronSccpProofRequest(Self.sampleTronProofRequestInput(
            sourceDomain: sccpDomainEthereum
        ))) { error in
            XCTAssertEqual(error as? TronSccpProverError, .invalidPublicInputs("sourceDomain"))
        }
        XCTAssertThrowsError(try buildTronSccpProofRequest(Self.sampleTronProofRequestInput(
            publicInputs: Self.sampleTronPublicInputs(targetDomain: sccpDomainTon)
        ))) { error in
            XCTAssertEqual(error as? TronSccpProverError, .invalidPublicInputs("publicInputs.targetDomain"))
        }
        XCTAssertThrowsError(try buildTronSccpProofRequest(Self.sampleTronProofRequestInput(
            destinationBindingHash: String(repeating: "00", count: 32)
        ))) { error in
            XCTAssertEqual(error as? TronSccpProverError, .zeroField("destinationBindingHash"))
        }
        XCTAssertThrowsError(try buildTronSccpProofRequest(Self.sampleTronProofRequestInput(
            backend: "debug-tron-backend"
        ))) { error in
            XCTAssertEqual(error as? TronSccpProverError, .invalidPublicInputs("backend"))
        }
        let wrongSourceBinding = TronSccpDestinationBinding(
            version: destinationBinding.version,
            sourceDomain: sccpDomainEthereum,
            targetDomain: destinationBinding.targetDomain,
            networkId: destinationBinding.networkId,
            verifierAddress: destinationBinding.verifierAddress,
            verifierCodeHash: destinationBinding.verifierCodeHash,
            verifierKeyHash: destinationBinding.verifierKeyHash,
            verifierBackend: destinationBinding.verifierBackend,
            proofFamily: destinationBinding.proofFamily,
            key: destinationBinding.key,
            hash: destinationBinding.hash
        )
        XCTAssertThrowsError(try TronSccpProofRequestInput(
            publicInputs: Self.sampleTronPublicInputs(),
            bundleBytes: Data([5, 6, 7]),
            statementHash: String(repeating: "56", count: 32),
            destinationBinding: wrongSourceBinding
        )) { error in
            XCTAssertEqual(error as? TronSccpProverError, .invalidPublicInputs("destinationBinding.sourceDomain"))
        }
    }

    func testTronProverRequiresLinkedProofEngine() async throws {
        let prover = TronSccpProver()

        do {
            _ = try await prover.prove(Self.sampleTronProofRequestInput())
            XCTFail("expected localProverUnavailable")
        } catch let error as TronSccpProverError {
            XCTAssertEqual(error, .localProverUnavailable)
        }
    }

    func testTronProverWrapsExternalProofBytes() async throws {
        let proofBytes = Self.sampleGroth16ProofBytes()
        var seenTronProofRequests: [TronSccpProofRequest] = []
        let prover = TronSccpProver { request in
            seenTronProofRequests.append(request)
            XCTAssertEqual(request.backend, sccpTronGroth16Bn254ProofBackendV1)
            XCTAssertEqual(request.publicSignalWords.count, 9)
            var callbackBundleBytes = request.bundleBytes
            callbackBundleBytes[callbackBundleBytes.startIndex] =
                callbackBundleBytes[callbackBundleBytes.startIndex] == 0 ? 1 : 0
            var callbackSourceProofBytes = request.sourceProofBytes
            if !callbackSourceProofBytes.isEmpty {
                callbackSourceProofBytes[callbackSourceProofBytes.startIndex] =
                    callbackSourceProofBytes[callbackSourceProofBytes.startIndex] == 0 ? 1 : 0
                XCTAssertEqual(request.sourceProofBytes, Data([9, 10]))
                XCTAssertNotEqual(callbackSourceProofBytes, request.sourceProofBytes)
            }
            var callbackPublicSignalWords = request.publicSignalWords
            callbackPublicSignalWords[0] = "0x" + String(repeating: "aa", count: 32)
            XCTAssertEqual(request.bundleBytes, Data([5, 6, 7]))
            XCTAssertNotEqual(callbackBundleBytes, request.bundleBytes)
            XCTAssertEqual(request.publicSignalWords.count, 9)
            XCTAssertNotEqual(callbackPublicSignalWords, request.publicSignalWords)
            return proofBytes
        }

        let sourceProofInput = try Self.sampleProductionTronProofRequestInput(sourceProofBytes: Data([9, 10]))
        let omittedSourceInput = try Self.sampleProductionTronProofRequestInput()
        let result = try await prover.prove(sourceProofInput)
        let omittedSourceResult = try await prover.prove(omittedSourceInput)
        XCTAssertTrue(omittedSourceResult.sourceProofBytes.isEmpty)
        XCTAssertEqual(
            seenTronProofRequests,
            [
                try buildTronSccpProofRequest(sourceProofInput),
                try buildTronSccpProofRequest(omittedSourceInput),
            ]
        )

        XCTAssertEqual(result.proofBytes, proofBytes)
        XCTAssertFalse(result.proofBase64.isEmpty)
        XCTAssertEqual(result.statementHash, "0x" + String(repeating: "56", count: 32))
        XCTAssertEqual(result.destinationBindingHash, sourceProofInput.destinationBinding?.hash)
        XCTAssertEqual(result.destinationBinding, sourceProofInput.destinationBinding)
        XCTAssertEqual(result.bundleBytes, Data([5, 6, 7]))
        XCTAssertEqual(result.sourceProofBytes, Data([9, 10]))
        XCTAssertTrue(result.requestHash.hasPrefix("0x"))
        XCTAssertEqual(result.requestHash.count, 66)
        XCTAssertTrue(result.envelopeHash.hasPrefix("0x"))
        XCTAssertEqual(result.envelopeHash.count, 66)

        let hashOnlyRequest = try buildTronSccpProofRequest(Self.sampleTronProofRequestInput(sourceProofBytes: Data([9, 10])))
        XCTAssertThrowsError(try wrapTronSccpProofResult(
            proofBytes: proofBytes,
            request: hashOnlyRequest
        )) { error in
            XCTAssertEqual(error as? TronSccpProverError, .invalidPublicInputs("request.destinationBinding"))
        }

        let request = try buildTronSccpProofRequest(sourceProofInput)
        XCTAssertThrowsError(try wrapTronSccpProofResult(
            proofBytes: Data([0, 0]),
            request: request
        )) { error in
            XCTAssertEqual(error as? TronSccpProverError, .allZeroProof)
        }
        XCTAssertThrowsError(try wrapTronSccpProofResult(
            proofBytes: Data([1, 2, 3, 4]),
            request: request
        )) { error in
            XCTAssertEqual(error as? TronSccpProverError, .invalidProofLength(4))
        }
        let mismatchedRequest = TronSccpProofRequest(
            version: request.version,
            backend: request.backend,
            sourceDomain: request.sourceDomain,
            targetDomain: request.targetDomain,
            publicInputs: request.publicInputs,
            publicInputsBytes: request.publicInputsBytes,
            publicSignalWords: request.publicSignalWords,
            bundleBytes: request.bundleBytes,
            sourceProofBytes: request.sourceProofBytes,
            proofContext: request.proofContext,
            statementHash: request.statementHash,
            destinationBindingHash: request.destinationBindingHash,
            requestHash: "0x" + String(repeating: "cc", count: 32),
            destinationBinding: request.destinationBinding
        )
        XCTAssertThrowsError(try wrapTronSccpProofResult(
            proofBytes: proofBytes,
            request: mismatchedRequest
        )) { error in
            XCTAssertEqual(error as? TronSccpProverError, .invalidPublicInputs("request"))
        }
    }

    func testTronProverResolvesWitnessProviderBeforeBuildingRequest() async throws {
        let proofBytes = Self.sampleGroth16ProofBytes()
        let witnessProvider = TronSourceProofWitnessProvider()
        let prover = TronSccpProver(
            witnessProvider: witnessProvider,
            proveFunction: { request in
                XCTAssertEqual(witnessProvider.resolveCount, 1)
                XCTAssertEqual(request.sourceProofBytes, Data([9, 10]))
                return proofBytes
            }
        )

        let result = try await prover.prove(try Self.sampleProductionTronProofRequestInput())

        XCTAssertEqual(witnessProvider.resolveCount, 1)
        XCTAssertEqual(result.sourceProofBytes, Data([9, 10]))
    }

    func testBuildsTronContractCallSubmission() throws {
        let proofBytes = Self.sampleGroth16ProofBytes()
        let request = try buildTronSccpProofRequest(try Self.sampleProductionTronProofRequestInput(
            sourceProofBytes: Data([9, 10])
        ))
        let proofResult = try wrapTronSccpProofResult(proofBytes: proofBytes, request: request)
        let submission = try buildTronSccpSubmission(TronSccpSubmissionInput(proofResult: proofResult))
        let directCallData = try tronSccpSubmitMessageProofCallData(
            proofBytes: proofBytes,
            publicInputs: proofResult.publicInputs,
            statementHash: proofResult.statementHash
        )

        XCTAssertEqual(submission.submissionKind, "contract_call")
        XCTAssertEqual(submission.platformPayload, "tron_contract_call")
        XCTAssertEqual(submission.envelopeEncoding, sccpTronContractCallAbiTupleV1)
        XCTAssertEqual(submission.functionSelector, sccpSubmitMessageProofSelectorV1)
        XCTAssertTrue(submission.callDataHex.hasPrefix(sccpSubmitMessageProofSelectorV1))
        XCTAssertEqual(submission.callData.count, 676)
        XCTAssertEqual(
            "0x" + String(repeating: "00", count: 30) + "0100",
            "0x" + submission.callData.subdata(in: 4..<36).hexEncodedString()
        )
        XCTAssertEqual(
            "0x" + String(repeating: "00", count: 30) + "0180",
            "0x" + submission.callData.subdata(in: 260..<292).hexEncodedString()
        )
        XCTAssertEqual(submission.publicInputWords, try tronSccpMessageTransparentPublicInputAbiWords(Self.sampleTronPublicInputs()))
        XCTAssertEqual(submission.publicSignalWords, proofResult.publicSignalWords)
        XCTAssertEqual(proofResult.bundleBytes, Data([5, 6, 7]))
        XCTAssertEqual(proofResult.sourceProofBytes, Data([9, 10]))
        XCTAssertEqual(proofResult.destinationBinding, request.destinationBinding)
        XCTAssertEqual(submission.envelopeBytes, submission.callData)
        XCTAssertEqual(directCallData, submission.callData)
        let destinationBinding = try sccpTronDestinationBinding(
            networkId: "0x" + String(repeating: "33", count: 32),
            verifierAddress: "TJRabPrwbZy45sbavfcjinPJC18kjpRTv8",
            verifierCodeHash: "0x" + String(repeating: "bb", count: 32),
            verifierKeyHash: "0x" + String(repeating: "cc", count: 32)
        )
        let bindingSubmission = try buildTronSccpSubmission(try TronSccpSubmissionInput(
            publicInputs: proofResult.publicInputs,
            proofBytes: proofBytes,
            statementHash: proofResult.statementHash,
            destinationBinding: destinationBinding
        ))
        XCTAssertEqual(bindingSubmission.destinationBindingHash, destinationBinding.hash)

        let omittedSourceProofResult = try wrapTronSccpProofResult(
            proofBytes: proofBytes,
            request: try buildTronSccpProofRequest(try Self.sampleProductionTronProofRequestInput())
        )
        XCTAssertTrue(omittedSourceProofResult.sourceProofBytes.isEmpty)

        var proofMismatch = proofBytes
        proofMismatch[4 * 32 + 31] = 9
        XCTAssertThrowsError(try buildTronSccpSubmission(TronSccpSubmissionInput(
            publicInputs: proofResult.publicInputs,
            proofBytes: proofMismatch,
            statementHash: proofResult.statementHash,
            destinationBindingHash: proofResult.destinationBindingHash,
            proofResult: proofResult
        ))) { error in
            XCTAssertEqual(error as? TronSccpProverError, .invalidPublicInputs("proofBytes.a"))
        }
        let wrongSourceBinding = TronSccpDestinationBinding(
            version: destinationBinding.version,
            sourceDomain: sccpDomainEthereum,
            targetDomain: destinationBinding.targetDomain,
            networkId: destinationBinding.networkId,
            verifierAddress: destinationBinding.verifierAddress,
            verifierCodeHash: destinationBinding.verifierCodeHash,
            verifierKeyHash: destinationBinding.verifierKeyHash,
            verifierBackend: destinationBinding.verifierBackend,
            proofFamily: destinationBinding.proofFamily,
            key: destinationBinding.key,
            hash: destinationBinding.hash
        )
        XCTAssertThrowsError(try TronSccpSubmissionInput(
            publicInputs: proofResult.publicInputs,
            proofBytes: proofBytes,
            statementHash: proofResult.statementHash,
            destinationBinding: wrongSourceBinding
        )) { error in
            XCTAssertEqual(error as? TronSccpProverError, .invalidPublicInputs("destinationBinding.sourceDomain"))
        }

        let tamperedEnvelopeProofResult = TronSccpProofResult(
            version: proofResult.version,
            backend: proofResult.backend,
            proofBytes: proofResult.proofBytes,
            proofBase64: proofResult.proofBase64,
            publicInputs: proofResult.publicInputs,
            publicSignalWords: proofResult.publicSignalWords,
            proofContext: proofResult.proofContext,
            statementHash: proofResult.statementHash,
            destinationBindingHash: proofResult.destinationBindingHash,
            requestHash: proofResult.requestHash,
            envelopeHash: "0x" + String(repeating: "aa", count: 32),
            destinationBinding: proofResult.destinationBinding
        )
        XCTAssertThrowsError(try buildTronSccpSubmission(TronSccpSubmissionInput(
            proofResult: tamperedEnvelopeProofResult
        ))) { error in
            XCTAssertEqual(error as? TronSccpProverError, .invalidPublicInputs("proofResult.envelopeHash"))
        }

        let tamperedBase64ProofResult = TronSccpProofResult(
            version: proofResult.version,
            backend: proofResult.backend,
            proofBytes: proofResult.proofBytes,
            proofBase64: "AAAA",
            publicInputs: proofResult.publicInputs,
            publicSignalWords: proofResult.publicSignalWords,
            proofContext: proofResult.proofContext,
            statementHash: proofResult.statementHash,
            destinationBindingHash: proofResult.destinationBindingHash,
            requestHash: proofResult.requestHash,
            envelopeHash: proofResult.envelopeHash,
            destinationBinding: proofResult.destinationBinding
        )
        XCTAssertThrowsError(try buildTronSccpSubmission(TronSccpSubmissionInput(
            proofResult: tamperedBase64ProofResult
        ))) { error in
            XCTAssertEqual(error as? TronSccpProverError, .invalidPublicInputs("proofResult.proofBase64"))
        }

        let staleRequestProofResult = TronSccpProofResult(
            version: proofResult.version,
            backend: proofResult.backend,
            proofBytes: proofResult.proofBytes,
            proofBase64: proofResult.proofBase64,
            publicInputs: proofResult.publicInputs,
            publicSignalWords: proofResult.publicSignalWords,
            bundleBytes: Data([5, 6, 8]),
            sourceProofBytes: proofResult.sourceProofBytes,
            proofContext: proofResult.proofContext,
            statementHash: proofResult.statementHash,
            destinationBindingHash: proofResult.destinationBindingHash,
            requestHash: proofResult.requestHash,
            envelopeHash: proofResult.envelopeHash,
            destinationBinding: proofResult.destinationBinding
        )
        XCTAssertThrowsError(try buildTronSccpSubmission(TronSccpSubmissionInput(
            proofResult: staleRequestProofResult
        ))) { error in
            XCTAssertEqual(error as? TronSccpProverError, .invalidPublicInputs("proofResult.requestHash"))
        }

        var mismatchedSignals = proofResult.publicSignalWords
        mismatchedSignals[0] = "0x" + String(repeating: "99", count: 32)
        XCTAssertThrowsError(try buildTronSccpSubmission(TronSccpSubmissionInput(
            publicInputs: proofResult.publicInputs,
            proofBytes: proofBytes,
            statementHash: proofResult.statementHash,
            destinationBindingHash: proofResult.destinationBindingHash,
            publicSignalWords: mismatchedSignals
        ))) { error in
            XCTAssertEqual(error as? TronSccpProverError, .invalidPublicInputs("publicSignalWords"))
        }

        XCTAssertThrowsError(try buildTronSccpSubmission(TronSccpSubmissionInput(
            publicInputs: Self.sampleTronPublicInputs(targetDomain: sccpDomainTon),
            proofBytes: proofBytes,
            statementHash: proofResult.statementHash,
            destinationBindingHash: proofResult.destinationBindingHash
        ))) { error in
            XCTAssertEqual(error as? TronSccpProverError, .invalidPublicInputs("publicInputs.targetDomain"))
        }
    }

    func testRejectsMalformedTronGroth16ProofTuple() throws {
        let request = try buildTronSccpProofRequest(try Self.sampleProductionTronProofRequestInput(
            sourceProofBytes: Data([9, 10])
        ))

        XCTAssertThrowsError(try wrapTronSccpProofResult(
            proofBytes: Self.sampleGroth16ProofBytes(overrides: [0: Self.abiWord(2)]),
            request: request
        )) { error in
            XCTAssertEqual(error as? TronSccpProverError, .invalidPublicInputs("proofBytes.version"))
        }
        XCTAssertThrowsError(try wrapTronSccpProofResult(
            proofBytes: Self.sampleGroth16ProofBytes(overrides: [4: Data(repeating: 0xff, count: 32)]),
            request: request
        )) { error in
            XCTAssertEqual(error as? TronSccpProverError, .invalidPublicInputs("proofBytes.a.x"))
        }
        XCTAssertThrowsError(try wrapTronSccpProofResult(
            proofBytes: Self.sampleGroth16ProofBytes(overrides: [
                4: Data(repeating: 0, count: 32),
                5: Data(repeating: 0, count: 32),
            ]),
            request: request
        )) { error in
            XCTAssertEqual(error as? TronSccpProverError, .invalidPublicInputs("proofBytes.a"))
        }
        XCTAssertThrowsError(try wrapTronSccpProofResult(
            proofBytes: Self.sampleGroth16ProofBytes(overrides: [
                6: Data(repeating: 0, count: 32),
                7: Data(repeating: 0, count: 32),
                8: Data(repeating: 0, count: 32),
                9: Data(repeating: 0, count: 32),
            ]),
            request: request
        )) { error in
            XCTAssertEqual(error as? TronSccpProverError, .invalidPublicInputs("proofBytes.b"))
        }
        XCTAssertThrowsError(try wrapTronSccpProofResult(
            proofBytes: Self.sampleGroth16ProofBytes(overrides: [
                10: Data(repeating: 0, count: 32),
                11: Data(repeating: 0, count: 32),
            ]),
            request: request
        )) { error in
            XCTAssertEqual(error as? TronSccpProverError, .invalidPublicInputs("proofBytes.c"))
        }
        XCTAssertThrowsError(try wrapTronSccpProofResult(
            proofBytes: Self.sampleGroth16ProofBytes(overrides: [6: Self.abiWord(4)]),
            request: request
        )) { error in
            XCTAssertEqual(error as? TronSccpProverError, .invalidPublicInputs("proofBytes.b"))
        }
        XCTAssertThrowsError(try wrapTronSccpProofResult(
            proofBytes: Self.sampleGroth16ProofBytes(overrides: [
                6: Self.abiWord(0),
                7: Self.abiWord(1),
                8: Data(hexString: "0cf32d3c49a2cb8a092f24ec3201e68dc299b6216e6321ee60573e3a7f596ea8")!,
                9: Data(hexString: "07bca656753ef8cbee60335acbffe3def91636952d4ab9eb0b839c7f3566c0e2")!,
            ]),
            request: request
        )) { error in
            XCTAssertEqual(error as? TronSccpProverError, .invalidPublicInputs("proofBytes.b"))
        }
        XCTAssertThrowsError(try wrapTronSccpProofResult(
            proofBytes: Self.sampleGroth16ProofBytes(overrides: [1: Self.repeatedWord(0x22)]),
            request: request
        )) { error in
            XCTAssertEqual(error as? TronSccpProverError, .invalidPublicInputs("proofBytes.messageId"))
        }
        XCTAssertThrowsError(try wrapTronSccpProofResult(
            proofBytes: Self.sampleGroth16ProofBytes(overrides: [2: Self.abiWord(999)]),
            request: request
        )) { error in
            XCTAssertEqual(error as? TronSccpProverError, .invalidPublicInputs("proofBytes.sourceDomain"))
        }
        XCTAssertThrowsError(try tronSccpSubmitMessageProofCallData(
            proofBytes: Self.sampleGroth16ProofBytes(overrides: [2: Self.abiWord(UInt64(sccpDomainEthereum))]),
            publicInputs: Self.sampleTronPublicInputs(),
            statementHash: String(repeating: "56", count: 32)
        )) { error in
            XCTAssertEqual(error as? TronSccpProverError, .invalidPublicInputs("proofBytes.sourceDomain"))
        }
        XCTAssertThrowsError(try tronSccpSubmitMessageProofCallData(
            proofBytes: Self.sampleGroth16ProofBytes(overrides: [2: Self.abiWord(UInt64(sccpDomainEthereum))]),
            publicInputs: Self.sampleTronPublicInputs(),
            statementHash: String(repeating: "56", count: 32),
            sourceDomain: sccpDomainEthereum
        )) { error in
            XCTAssertEqual(error as? TronSccpProverError, .invalidPublicInputs("sourceDomain"))
        }
        XCTAssertThrowsError(try buildTronSccpSubmission(TronSccpSubmissionInput(
            publicInputs: Self.sampleTronPublicInputs(),
            proofBytes: Self.sampleGroth16ProofBytes(overrides: [3: Self.repeatedWord(0x44)]),
            statementHash: String(repeating: "56", count: 32),
            destinationBindingHash: String(repeating: "78", count: 32)
        ))) { error in
            XCTAssertEqual(error as? TronSccpProverError, .invalidPublicInputs("proofBytes.commitmentRoot"))
        }
    }

    func testEvmProofRequestBindsPublicSignalsAndRelayContext() throws {
        let request = try buildEvmSccpProofRequest(Self.sampleEvmProofRequestInput(sourceProofBytes: Data([9, 10])))
        let expectedSignals = try sccpGroth16Bn254PublicSignalWords(
            publicInputs: Self.sampleEvmPublicInputs(),
            sourceDomain: sccpDomainSora,
            statementHash: String(repeating: "56", count: 32),
            destinationBindingHash: String(repeating: "78", count: 32)
        )

        XCTAssertEqual(request.backend, sccpEvmGroth16Bn254ProofBackendV1)
        XCTAssertEqual(request.sourceDomain, sccpDomainSora)
        XCTAssertEqual(request.targetDomain, sccpDomainEthereum)
        XCTAssertEqual(request.publicSignalWords, expectedSignals)
        XCTAssertEqual(request.publicSignalWords[2], "0x2eb6b5dbab56255a979f433862429637ba1e8251106271606f0a279f593d7a39")
        XCTAssertEqual(request.proofContext.statementHash, "0x" + String(repeating: "56", count: 32))
        XCTAssertEqual(request.proofContext.destinationBindingHash, "0x" + String(repeating: "78", count: 32))
        XCTAssertEqual(
            request.requestHash,
            "0xfb990c2ffdf826c9beb0e74105b060af467570720a1382b48abc42d32850f5ea"
        )

        let destinationBinding = try sccpEvmDestinationBinding(
            networkId: "0x" + String(repeating: "33", count: 32),
            verifierAddress: "0x" + String(repeating: "11", count: 20),
            bridgeAddress: "0x" + String(repeating: "22", count: 20),
            verifierCodeHash: "0x" + String(repeating: "bb", count: 32),
            verifierKeyHash: "0x" + String(repeating: "cc", count: 32)
        )
        let boundRequest = try buildEvmSccpProofRequest(try EvmSccpProofRequestInput(
            publicInputs: Self.sampleEvmPublicInputs(),
            bundleBytes: Data([5, 6, 7]),
            sourceProofBytes: Data([9, 10]),
            statementHash: String(repeating: "56", count: 32),
            destinationBinding: destinationBinding
        ))
        XCTAssertEqual(boundRequest.destinationBindingHash, destinationBinding.hash)
        XCTAssertEqual(boundRequest.destinationBinding, destinationBinding)
        XCTAssertNotEqual(boundRequest.requestHash, request.requestHash)

        let forgedHashBinding = EvmSccpDestinationBinding(
            version: destinationBinding.version,
            sourceDomain: destinationBinding.sourceDomain,
            targetDomain: destinationBinding.targetDomain,
            networkId: destinationBinding.networkId,
            verifierAddress: destinationBinding.verifierAddress,
            bridgeAddress: destinationBinding.bridgeAddress,
            verifierCodeHash: destinationBinding.verifierCodeHash,
            verifierKeyHash: destinationBinding.verifierKeyHash,
            verifierBackend: destinationBinding.verifierBackend,
            proofFamily: destinationBinding.proofFamily,
            key: destinationBinding.key,
            hash: "0x" + String(repeating: "a7", count: 32)
        )
        XCTAssertThrowsError(try EvmSccpProofRequestInput(
            publicInputs: Self.sampleEvmPublicInputs(),
            bundleBytes: Data([5, 6, 7]),
            statementHash: String(repeating: "56", count: 32),
            destinationBinding: forgedHashBinding
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("destinationBinding"))
        }

        let bscRequest = try buildEvmSccpProofRequest(Self.sampleEvmProofRequestInput(
            publicInputs: Self.sampleEvmPublicInputs(targetDomain: sccpDomainBsc),
            sourceProofBytes: Data([9, 10])
        ))
        XCTAssertEqual(bscRequest.targetDomain, sccpDomainBsc)
        XCTAssertNotEqual(request.publicSignalWords[2], bscRequest.publicSignalWords[2])
        XCTAssertNotEqual(request.requestHash, bscRequest.requestHash)
        let shiftedSplitRequest = try buildEvmSccpProofRequest(Self.sampleEvmProofRequestInput(
            bundleBytes: Data([5, 6, 7, 9]),
            sourceProofBytes: Data([10])
        ))
        XCTAssertNotEqual(request.requestHash, shiftedSplitRequest.requestHash)
        let artifactRequest = try buildEvmSccpProofRequest(Self.sampleEvmProofRequestInput(
            sourceProofBytes: Data([9, 10]),
            proofArtifactHash: String(repeating: "91", count: 32),
            provingKeyHash: String(repeating: "92", count: 32)
        ))
        XCTAssertEqual(artifactRequest.proofArtifactHash, "0x" + String(repeating: "91", count: 32))
        XCTAssertEqual(artifactRequest.provingKeyHash, "0x" + String(repeating: "92", count: 32))
        XCTAssertNotEqual(artifactRequest.requestHash, request.requestHash)
        XCTAssertThrowsError(try buildEvmSccpProofRequest(Self.sampleEvmProofRequestInput(
            proofArtifactHash: String(repeating: "91", count: 32)
        ))) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("proofArtifactHash/provingKeyHash"))
        }
        XCTAssertThrowsError(try buildEvmSccpProofRequest(Self.sampleEvmProofRequestInput(
            proofArtifactHash: String(repeating: "00", count: 32),
            provingKeyHash: String(repeating: "92", count: 32)
        ))) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .zeroField("proofArtifactHash"))
        }
        XCTAssertThrowsError(try buildEvmSccpProofRequest(Self.sampleEvmProofRequestInput(statementHash: ""))) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidHex32("statementHash"))
        }
        XCTAssertThrowsError(try buildEvmSccpProofRequest(Self.sampleEvmProofRequestInput(
            publicInputs: Self.sampleEvmPublicInputs(payloadHash: " " + String(repeating: "22", count: 32))
        ))) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidHex32("payloadHash"))
        }
        XCTAssertThrowsError(try buildEvmSccpProofRequest(Self.sampleEvmProofRequestInput(
            statementHash: String(repeating: "56", count: 32) + "\n"
        ))) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidHex32("statementHash"))
        }
        XCTAssertThrowsError(try buildEvmSccpProofRequest(Self.sampleEvmProofRequestInput(
            publicInputs: Self.sampleEvmPublicInputs(finalityHeight: 0)
        ))) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("publicInputs.finalityHeight"))
        }
        XCTAssertThrowsError(try buildEvmSccpProofRequest(Self.sampleEvmProofRequestInput(
            publicInputs: Self.sampleEvmPublicInputs(),
            sourceDomain: sccpDomainEthereum
        ))) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("sourceDomain"))
        }
        XCTAssertThrowsError(try buildEvmSccpProofRequest(Self.sampleEvmProofRequestInput(
            publicInputs: Self.sampleEvmPublicInputs(),
            sourceDomain: sccpDomainTon
        ))) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("sourceDomain"))
        }
        XCTAssertThrowsError(try buildEvmSccpProofRequest(Self.sampleEvmProofRequestInput(
            publicInputs: Self.sampleEvmPublicInputs(targetDomain: sccpDomainTon)
        ))) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("publicInputs.targetDomain"))
        }
        XCTAssertThrowsError(try buildEvmSccpProofRequest(Self.sampleEvmProofRequestInput(
            destinationBindingHash: String(repeating: "00", count: 32)
        ))) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .zeroField("destinationBindingHash"))
        }
        XCTAssertThrowsError(try buildEvmSccpProofRequest(Self.sampleEvmProofRequestInput(
            bundleBytes: Data()
        ))) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("bundleBytes"))
        }
        XCTAssertThrowsError(try buildEvmSccpProofRequest(Self.sampleEvmProofRequestInput(
            backend: "debug-evm-backend"
        ))) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("backend"))
        }
        let bscDestinationBinding = try sccpEvmDestinationBinding(
            targetDomain: sccpDomainBsc,
            networkId: "0x" + String(repeating: "33", count: 32),
            verifierAddress: "0x" + String(repeating: "11", count: 20),
            bridgeAddress: "0x" + String(repeating: "22", count: 20),
            verifierCodeHash: "0x" + String(repeating: "bb", count: 32),
            verifierKeyHash: "0x" + String(repeating: "cc", count: 32)
        )
        XCTAssertThrowsError(try EvmSccpProofRequestInput(
            publicInputs: Self.sampleEvmPublicInputs(),
            bundleBytes: Data([5, 6, 7]),
            statementHash: String(repeating: "56", count: 32),
            destinationBinding: bscDestinationBinding
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("destinationBinding.targetDomain"))
        }
    }

    func testEthereumMainnetSccpFacadeRequiresChainId1AndEthTarget() async throws {
        try EthereumMainnetSccp.requireMainnetChainId(1)
        XCTAssertThrowsError(try EthereumMainnetSccp.requireMainnetChainId(56)) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("eth_chainId"))
        }

        let binding = try EthereumMainnetSccp.destinationBinding(
            verifierAddress: "0x" + String(repeating: "11", count: 20),
            bridgeAddress: "0x" + String(repeating: "22", count: 20),
            verifierCodeHash: "0x" + String(repeating: "bb", count: 32),
            verifierKeyHash: "0x" + String(repeating: "cc", count: 32)
        )
        XCTAssertEqual(binding.networkId, sccpEthereumMainnetNetworkId)
        XCTAssertEqual(binding.sourceDomain, sccpDomainSora)
        XCTAssertEqual(binding.targetDomain, sccpDomainEthereum)
        XCTAssertEqual(
            binding.hash,
            try sccpEthereumMainnetDestinationBindingHash(
                verifierAddress: "0x" + String(repeating: "11", count: 20),
                bridgeAddress: "0x" + String(repeating: "22", count: 20),
                verifierCodeHash: "0x" + String(repeating: "bb", count: 32),
                verifierKeyHash: "0x" + String(repeating: "cc", count: 32)
            )
        )

        let input = try EvmSccpProofRequestInput(
            publicInputs: Self.sampleEvmPublicInputs(targetDomain: sccpDomainEthereum),
            bundleBytes: Data([5, 6, 7]),
            sourceProofBytes: Data([9, 10]),
            statementHash: String(repeating: "56", count: 32),
            destinationBinding: binding
        )
        let facade = EthereumMainnetSccp()
        let ethRequest = try await facade.buildOutboundProofRequest(input)
        XCTAssertEqual(ethRequest.targetDomain, sccpDomainEthereum)
        XCTAssertEqual(ethRequest.destinationBindingHash, binding.hash)
        let nativeProverBundle = try Self.sampleEthereumNativeEvmProverBundle(
            destinationBindingHash: binding.hash
        )
        let parsedNativeProverBundle = try EthereumMainnetNativeEvmProverBundle(
            jsonString: Self.sampleEthereumNativeEvmProverBundleJson(destinationBindingHash: binding.hash),
            expectedDestinationBindingHash: binding.hash
        )
        XCTAssertEqual(parsedNativeProverBundle, nativeProverBundle)
        XCTAssertEqual(parsedNativeProverBundle.proofArtifact, "artifacts/eth-mainnet/proof-artifact.bin")
        XCTAssertEqual(parsedNativeProverBundle.provingKey, "artifacts/eth-mainnet/proving-key.bin")
        XCTAssertEqual(parsedNativeProverBundle.verifierKey, "artifacts/eth-mainnet/verifier-key.bin")
        XCTAssertEqual(
            parsedNativeProverBundle.nativeProverSelfTestArtifact,
            "artifacts/eth-mainnet/native-prover-self-test.json"
        )
        XCTAssertEqual(
            parsedNativeProverBundle.nativeSdkArtifacts.first { $0.sdk == "swift" }?.implementationArtifact,
            "artifacts/eth-mainnet/swift-implementation.bin"
        )
        let parityJson = Self.sampleEthereumNativeEvmProverParityFixtureJson(
            nativeProverBundle: nativeProverBundle
        )
        let parityFixture = try EthereumMainnetNativeEvmProverParityFixture(
            jsonString: parityJson,
            nativeProverBundle: nativeProverBundle
        )
        XCTAssertEqual(parityFixture.schema, sccpEthNativeEvmProverParityFixtureSchemaV1)
        XCTAssertEqual(parityFixture.destinationBindingHash, binding.hash)
        XCTAssertEqual(parityFixture.publicSignalWords.count, 9)
        XCTAssertEqual(
            parityFixture.sdkResults["swift"]?.toriiSubmitPayloadHash,
            parityFixture.toriiSubmitPayloadHash
        )
        let driftedParityFixture = Self.sampleEthereumNativeEvmProverParityFixtureJson(
            nativeProverBundle: nativeProverBundle,
            swiftCalldataHash: "0x" + String(repeating: "96", count: 32)
        )
        XCTAssertThrowsError(try EthereumMainnetNativeEvmProverParityFixture(
            jsonString: driftedParityFixture,
            nativeProverBundle: nativeProverBundle
        )) { error in
            XCTAssertEqual(
                error as? EvmSccpProverError,
                .invalidPublicInputs("nativeProverParityFixture.sdkResults.swift")
            )
        }
        let duplicateParityFixture = parityJson.replacingOccurrences(
            of: "\"schema\": \"\(sccpEthNativeEvmProverParityFixtureSchemaV1)\"",
            with: """
            "schema": "forged",
                  "schema": "\(sccpEthNativeEvmProverParityFixtureSchemaV1)"
            """
        )
        XCTAssertThrowsError(try EthereumMainnetNativeEvmProverParityFixture(
            jsonString: duplicateParityFixture,
            nativeProverBundle: nativeProverBundle
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("nativeProverParityFixture.duplicateJsonKey"))
        }
        let selfTestJson = Self.sampleEthereumNativeEvmProverSelfTestFixtureJson(
            nativeProverBundle: nativeProverBundle
        )
        let selfTestFixture = try EthereumMainnetNativeEvmProverSelfTestFixture(
            jsonString: selfTestJson,
            nativeProverBundle: nativeProverBundle
        )
        XCTAssertEqual(selfTestFixture.schema, sccpEthNativeEvmProverSelfTestSchemaV1)
        XCTAssertEqual(selfTestFixture.destinationBindingHash, binding.hash)
        XCTAssertEqual(selfTestFixture.publicSignalWords.count, 9)
        XCTAssertEqual(selfTestFixture.sdkResults["swift"]?.proofHash, selfTestFixture.proofHash)
        let driftedSelfTestFixture = Self.sampleEthereumNativeEvmProverSelfTestFixtureJson(
            nativeProverBundle: nativeProverBundle,
            swiftProofHash: "0x" + String(repeating: "97", count: 32)
        )
        XCTAssertThrowsError(try EthereumMainnetNativeEvmProverSelfTestFixture(
            jsonString: driftedSelfTestFixture,
            nativeProverBundle: nativeProverBundle
        )) { error in
            XCTAssertEqual(
                error as? EvmSccpProverError,
                .invalidPublicInputs("nativeProverSelfTestFixture.sdkResults.swift")
            )
        }
        let duplicateSelfTestFixture = selfTestJson.replacingOccurrences(
            of: "\"schema\": \"\(sccpEthNativeEvmProverSelfTestSchemaV1)\"",
            with: """
            "schema": "forged",
                  "schema": "\(sccpEthNativeEvmProverSelfTestSchemaV1)"
            """
        )
        XCTAssertThrowsError(try EthereumMainnetNativeEvmProverSelfTestFixture(
            jsonString: duplicateSelfTestFixture,
            nativeProverBundle: nativeProverBundle
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("nativeProverSelfTestFixture.duplicateJsonKey"))
        }
        XCTAssertThrowsError(try EthereumMainnetNativeEvmProverBundle(
            jsonString: Self.sampleEthereumNativeEvmProverBundleJson(
                destinationBindingHash: binding.hash,
                noWasm: false
            ),
            expectedDestinationBindingHash: binding.hash
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("nativeProverBundle.noWasm"))
        }
        XCTAssertThrowsError(try EthereumMainnetNativeEvmProverBundle(
            jsonString: Self.sampleEthereumNativeEvmProverBundleJson(
                destinationBindingHash: "0x" + String(repeating: "95", count: 32)
            ),
            expectedDestinationBindingHash: binding.hash
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("nativeProverBundle.destinationBindingHash"))
        }
        let noncanonicalDomainManifest = Self.sampleEthereumNativeEvmProverBundleJson(
            destinationBindingHash: binding.hash
        ).replacingOccurrences(
            of: "\"domain\": \(sccpDomainEthereum)",
            with: "\"domain\": \"01\""
        )
        XCTAssertThrowsError(try EthereumMainnetNativeEvmProverBundle(
            jsonString: noncanonicalDomainManifest,
            expectedDestinationBindingHash: binding.hash
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("domain"))
        }
        let paddedSdkManifest = Self.sampleEthereumNativeEvmProverBundleJson(
            destinationBindingHash: binding.hash
        ).replacingOccurrences(of: "\"sdk\": \"swift\"", with: "\"sdk\": \" swift \"")
        XCTAssertThrowsError(try EthereumMainnetNativeEvmProverBundle(
            jsonString: paddedSdkManifest,
            expectedDestinationBindingHash: binding.hash
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("nativeSdkArtifacts.sdk"))
        }
        let duplicateJsonKeyManifest = Self.sampleEthereumNativeEvmProverBundleJson(
            destinationBindingHash: binding.hash
        ).replacingOccurrences(
            of: "\"bundle_id\": \"\(sccpEthNativeEvmProverBundleIdV1)\"",
            with: """
            "bundle_id": "forged",
                      "bundle_id": "\(sccpEthNativeEvmProverBundleIdV1)"
            """
        )
        XCTAssertThrowsError(try EthereumMainnetNativeEvmProverBundle(
            jsonString: duplicateJsonKeyManifest,
            expectedDestinationBindingHash: binding.hash
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("nativeProverBundle.duplicateJsonKey"))
        }
        XCTAssertThrowsError(try EthereumMainnetNativeEvmProverBundle(
            jsonString: Self.sampleEthereumNativeEvmProverBundleJson(
                destinationBindingHash: binding.hash,
                proofArtifact: "../proof-artifact.bin"
            ),
            expectedDestinationBindingHash: binding.hash
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("proofArtifact"))
        }
        XCTAssertThrowsError(try EthereumMainnetNativeEvmProverBundle(
            jsonString: Self.sampleEthereumNativeEvmProverBundleJson(
                destinationBindingHash: binding.hash,
                proofArtifact: "ipfs:proof-artifact.bin"
            ),
            expectedDestinationBindingHash: binding.hash
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("proofArtifact"))
        }
        XCTAssertThrowsError(try EthereumMainnetNativeEvmProverBundle(
            jsonString: Self.sampleEthereumNativeEvmProverBundleJson(
                destinationBindingHash: binding.hash,
                proofArtifact: "artifacts/eth-mainnet/proof.wasm"
            ),
            expectedDestinationBindingHash: binding.hash
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("proofArtifact"))
        }
        let unknownManifestField = Self.sampleEthereumNativeEvmProverBundleJson(
            destinationBindingHash: binding.hash
        ).replacingOccurrences(
            of: "\"audit_hashes\":",
            with: "\"experimental_manifest_note\": true,\n          \"audit_hashes\":"
        )
        XCTAssertThrowsError(try EthereumMainnetNativeEvmProverBundle(
            jsonString: unknownManifestField,
            expectedDestinationBindingHash: binding.hash
        )) { error in
            XCTAssertEqual(
                error as? EvmSccpProverError,
                .invalidPublicInputs("nativeProverBundle.experimental_manifest_note")
            )
        }
        let duplicateManifestAlias = Self.sampleEthereumNativeEvmProverBundleJson(
            destinationBindingHash: binding.hash
        ).replacingOccurrences(
            of: "\"proof_artifact_hash\": \"0x\(String(repeating: "91", count: 32))\"",
            with: """
            "proofArtifactHash": "0x\(String(repeating: "91", count: 32))",
                      "proof_artifact_hash": "0x\(String(repeating: "91", count: 32))"
            """
        )
        XCTAssertThrowsError(try EthereumMainnetNativeEvmProverBundle(
            jsonString: duplicateManifestAlias,
            expectedDestinationBindingHash: binding.hash
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("proofArtifactHash"))
        }
        let unknownArtifactField = Self.sampleEthereumNativeEvmProverBundleJson(
            destinationBindingHash: binding.hash
        ).replacingOccurrences(
            of: "\"implementation_hash\":",
            with: "\"experimental_manifest_note\": true,\n                  \"implementation_hash\":"
        )
        XCTAssertThrowsError(try EthereumMainnetNativeEvmProverBundle(
            jsonString: unknownArtifactField,
            expectedDestinationBindingHash: binding.hash
        )) { error in
            XCTAssertEqual(
                error as? EvmSccpProverError,
                .invalidPublicInputs("nativeSdkArtifacts[0].experimental_manifest_note")
            )
        }
        let noncanonicalAuditManifest = Self.sampleEthereumNativeEvmProverBundleJson(
            destinationBindingHash: binding.hash
        ).replacingOccurrences(
            of: "\"0x\(String(repeating: "a1", count: 32))\"",
            with: "\"0x\(String(repeating: "A1", count: 32))\""
        )
        XCTAssertThrowsError(try EthereumMainnetNativeEvmProverBundle(
            jsonString: noncanonicalAuditManifest,
            expectedDestinationBindingHash: binding.hash
        )) { error in
            XCTAssertEqual(
                error as? EvmSccpProverError,
                .invalidHex32("auditHashes.circuit_security_audit")
            )
        }
        let replayedAuditManifest = Self.sampleEthereumNativeEvmProverBundleJson(
            destinationBindingHash: binding.hash
        ).replacingOccurrences(
            of: "\"0x\(String(repeating: "a1", count: 32))\"",
            with: "\"0x\(String(repeating: "91", count: 32))\""
        )
        XCTAssertThrowsError(try EthereumMainnetNativeEvmProverBundle(
            jsonString: replayedAuditManifest,
            expectedDestinationBindingHash: binding.hash
        )) { error in
            XCTAssertEqual(
                error as? EvmSccpProverError,
                .invalidPublicInputs("auditHashes.circuit_security_audit")
            )
        }
        let bundledFacade = EthereumMainnetSccp(nativeProverBundle: nativeProverBundle)
        let bundledRequest = try await bundledFacade.buildOutboundProofRequest(input)
        XCTAssertEqual(bundledRequest.proofArtifactHash, "0x" + String(repeating: "91", count: 32))
        XCTAssertEqual(bundledRequest.provingKeyHash, "0x" + String(repeating: "92", count: 32))
        XCTAssertNotEqual(bundledRequest.requestHash, ethRequest.requestHash)
        XCTAssertEqual(
            try nativeProverBundle.applying(to: input).proofArtifactHash,
            "0x" + String(repeating: "91", count: 32)
        )
        let verifierKeyMismatchedBundle = try Self.sampleEthereumNativeEvmProverBundle(
            destinationBindingHash: binding.hash,
            verifierKeyHash: "0x" + String(repeating: "dd", count: 32)
        )
        XCTAssertThrowsError(try verifierKeyMismatchedBundle.applying(to: input)) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("nativeProverBundle.verifierKeyHash"))
        }
        let proofArtifactBytes = Self.nativeEvmProverArtifactBytes("swift proof artifact v1")
        let provingKeyBytes = Self.nativeEvmProverArtifactBytes("swift proving key v1")
        let verifierKeyBytes = Self.nativeEvmProverArtifactBytes("swift verifier key v1")
        let implementationBytes = Self.nativeEvmProverArtifactBytes("swift implementation artifact v1")
        let proofArtifactHash = Self.sha256Hex(proofArtifactBytes)
        let provingKeyHash = Self.sha256Hex(provingKeyBytes)
        let verifierKeyHash = Self.sha256Hex(verifierKeyBytes)
        let implementationHash = Self.sha256Hex(implementationBytes)
        let artifactBinding = try EthereumMainnetSccp.destinationBinding(
            verifierAddress: "0x" + String(repeating: "11", count: 20),
            bridgeAddress: "0x" + String(repeating: "22", count: 20),
            verifierCodeHash: "0x" + String(repeating: "bb", count: 32),
            verifierKeyHash: verifierKeyHash
        )
        let artifactInput = try EvmSccpProofRequestInput(
            publicInputs: Self.sampleEvmPublicInputs(targetDomain: sccpDomainEthereum),
            bundleBytes: Data([5, 6, 7]),
            sourceProofBytes: Data([9, 10]),
            statementHash: String(repeating: "56", count: 32),
            destinationBinding: artifactBinding
        )
        let draftVerifiedBundle = try EthereumMainnetNativeEvmProverBundle(
            proofArtifact: "artifacts/eth-mainnet/proof-artifact.bin",
            proofArtifactHash: proofArtifactHash,
            provingKey: "artifacts/eth-mainnet/proving-key.bin",
            provingKeyHash: provingKeyHash,
            verifierKey: "artifacts/eth-mainnet/verifier-key.bin",
            verifierKeyHash: verifierKeyHash,
            destinationBindingHash: artifactBinding.hash,
            nativeSdkArtifacts: try sccpEthNativeEvmProverRequiredImplementationsV1
                .sorted { $0.key < $1.key }
                .enumerated()
                .map { index, entry in
                    try EthereumMainnetNativeEvmProverBundleSdkArtifact(
                        sdk: entry.key,
                        implementation: entry.value,
                        proofArtifactHash: proofArtifactHash,
                        provingKeyHash: provingKeyHash,
                        implementationArtifact: "artifacts/eth-mainnet/\(entry.key)-implementation.bin",
                        implementationHash: entry.key == "swift"
                            ? implementationHash
                            : "0x" + String(
                                repeating: String(format: "%02x", index + 1),
                                count: 32
                            )
                    )
                },
            nativeProverSelfTestArtifact: "artifacts/eth-mainnet/native-prover-self-test.json",
            auditHashes: Self.sampleEthereumNativeEvmProverAuditHashes(),
            expectedDestinationBindingHash: artifactBinding.hash
        )
        let parityFixtureBytes = Data(Self.sampleEthereumNativeEvmProverParityFixtureJson(
            nativeProverBundle: draftVerifiedBundle
        ).utf8)
        let parityFixtureHash = Self.sha256Hex(parityFixtureBytes)
        let selfTestFixtureBytes = Data(Self.sampleEthereumNativeEvmProverSelfTestFixtureJson(
            nativeProverBundle: draftVerifiedBundle
        ).utf8)
        let selfTestFixtureHash = Self.sha256Hex(selfTestFixtureBytes)
        var verifiedAuditHashes = Self.sampleEthereumNativeEvmProverAuditHashes()
        verifiedAuditHashes["cross_sdk_fixture_parity"] = parityFixtureHash
        verifiedAuditHashes["native_prover_self_test"] = selfTestFixtureHash
        let verifiedBundle = try EthereumMainnetNativeEvmProverBundle(
            proofArtifact: "artifacts/eth-mainnet/proof-artifact.bin",
            proofArtifactHash: proofArtifactHash,
            provingKey: "artifacts/eth-mainnet/proving-key.bin",
            provingKeyHash: provingKeyHash,
            verifierKey: "artifacts/eth-mainnet/verifier-key.bin",
            verifierKeyHash: verifierKeyHash,
            destinationBindingHash: artifactBinding.hash,
            nativeSdkArtifacts: draftVerifiedBundle.nativeSdkArtifacts,
            crossSdkFixtureParityArtifact: "artifacts/eth-mainnet/cross-sdk-fixture-parity.json",
            nativeProverSelfTestArtifact: "artifacts/eth-mainnet/native-prover-self-test.json",
            auditHashes: verifiedAuditHashes,
            expectedDestinationBindingHash: artifactBinding.hash
        )
        let verifiedArtifacts = try verifiedBundle.verifiedArtifacts(
            proofArtifactBytes: proofArtifactBytes,
            provingKeyBytes: provingKeyBytes,
            verifierKeyBytes: verifierKeyBytes,
            sdk: "swift",
            implementationBytes: implementationBytes,
            crossSdkFixtureParityBytes: parityFixtureBytes,
            nativeProverSelfTestBytes: selfTestFixtureBytes
        )
        XCTAssertEqual(verifiedArtifacts.hashAlgorithm, sccpNativeEvmProverArtifactHashAlgorithmV1)
        XCTAssertEqual(verifiedArtifacts.proofArtifactHash, proofArtifactHash)
        XCTAssertEqual(verifiedArtifacts.provingKeyHash, provingKeyHash)
        XCTAssertEqual(verifiedArtifacts.verifierKeyHash, verifierKeyHash)
        XCTAssertEqual(verifiedArtifacts.crossSdkFixtureParityHash, parityFixtureHash)
        XCTAssertEqual(
            verifiedArtifacts.crossSdkFixtureParity?.calldataHash,
            "0x" + String(repeating: "d3", count: 32)
        )
        XCTAssertEqual(verifiedArtifacts.nativeProverSelfTestHash, selfTestFixtureHash)
        XCTAssertEqual(
            verifiedArtifacts.nativeProverSelfTest?.proofHash,
            "0x" + String(repeating: "e4", count: 32)
        )
        XCTAssertEqual(verifiedArtifacts.implementation, "native-swift")
        XCTAssertEqual(verifiedArtifacts.implementationHash, implementationHash)
        let swiftImplementationArtifact = try XCTUnwrap(
            verifiedBundle.nativeSdkArtifacts.first { $0.sdk == "swift" }?.implementationArtifact
        )
        let artifactBytesByPath = [
            try XCTUnwrap(verifiedBundle.proofArtifact): proofArtifactBytes,
            try XCTUnwrap(verifiedBundle.provingKey): provingKeyBytes,
            try XCTUnwrap(verifiedBundle.verifierKey): verifierKeyBytes,
            try XCTUnwrap(verifiedBundle.crossSdkFixtureParityArtifact): parityFixtureBytes,
            try XCTUnwrap(verifiedBundle.nativeProverSelfTestArtifact): selfTestFixtureBytes,
            swiftImplementationArtifact: implementationBytes
        ]
        let verifiedFromResolver = try verifiedBundle.verifiedArtifacts(sdk: "swift") { path in
            guard let bytes = artifactBytesByPath[path] else {
                throw EvmSccpProverError.invalidPublicInputs(path)
            }
            return bytes
        }
        XCTAssertEqual(verifiedFromResolver.implementationHash, implementationHash)
        XCTAssertEqual(verifiedFromResolver.crossSdkFixtureParityHash, parityFixtureHash)
        XCTAssertEqual(verifiedFromResolver.nativeProverSelfTestHash, selfTestFixtureHash)
        XCTAssertThrowsError(try verifiedBundle.verifiedArtifacts(sdk: " swift ") { path in
            artifactBytesByPath[path] ?? Data()
        }) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("sdk"))
        }
        XCTAssertThrowsError(try verifiedBundle.verifiedArtifacts(sdk: "swift") { path in
            if path == verifiedBundle.crossSdkFixtureParityArtifact {
                throw EvmSccpProverError.invalidPublicInputs("crossSdkFixtureParityArtifact")
            }
            return artifactBytesByPath[path] ?? Data()
        }) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("crossSdkFixtureParityArtifact"))
        }
        XCTAssertThrowsError(try verifiedBundle.verifiedArtifacts(sdk: "swift") { path in
            if path == verifiedBundle.nativeProverSelfTestArtifact {
                throw EvmSccpProverError.invalidPublicInputs("nativeProverSelfTestArtifact")
            }
            return artifactBytesByPath[path] ?? Data()
        }) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("nativeProverSelfTestArtifact"))
        }
        var missingArtifactsProverCalled = false
        let missingArtifactsFacade = EthereumMainnetSccp(proveFunction: { _ in
            missingArtifactsProverCalled = true
            return Self.sampleGroth16ProofBytes()
        })
        do {
            _ = try await missingArtifactsFacade.proveOutboundToEthereum(input)
            XCTFail("Ethereum outbound prover must require verified native artifacts")
        } catch let error as EvmSccpProverError {
            XCTAssertEqual(error, .invalidPublicInputs("nativeProverArtifacts"))
        }
        XCTAssertFalse(missingArtifactsProverCalled)
        var artifactBoundRequest: EvmSccpProofRequest?
        var artifactBoundSelfTestCalled = false
        let artifactBoundFacade = EthereumMainnetSccp(
            proveFunction: { request in
                artifactBoundRequest = request
                XCTAssertEqual(request.proofArtifactHash, proofArtifactHash)
                XCTAssertEqual(request.provingKeyHash, provingKeyHash)
                return Self.sampleGroth16ProofBytes()
            },
            nativeProverSelfTestFunction: { fixture, expected, artifacts in
                artifactBoundSelfTestCalled = true
                XCTAssertEqual(fixture.proofHash, "0x" + String(repeating: "e4", count: 32))
                XCTAssertEqual(artifacts.nativeProverSelfTestHash, selfTestFixtureHash)
                return expected
            },
            nativeProverArtifacts: verifiedArtifacts
        )
        let preflightResult = try await artifactBoundFacade.runNativeProverSelfTest()
        XCTAssertTrue(artifactBoundSelfTestCalled)
        XCTAssertEqual(preflightResult.proofHash, "0x" + String(repeating: "e4", count: 32))
        artifactBoundSelfTestCalled = false
        let artifactBoundResult = try await artifactBoundFacade.proveOutboundToEthereum(artifactInput)
        XCTAssertTrue(artifactBoundSelfTestCalled)
        XCTAssertEqual(artifactBoundRequest?.proofArtifactHash, proofArtifactHash)
        XCTAssertEqual(artifactBoundResult.proofArtifactHash, proofArtifactHash)
        XCTAssertEqual(artifactBoundResult.provingKeyHash, provingKeyHash)
        var missingSelfTestHookProverCalled = false
        let missingSelfTestHookFacade = EthereumMainnetSccp(
            proveFunction: { _ in
                missingSelfTestHookProverCalled = true
                return Self.sampleGroth16ProofBytes()
            },
            nativeProverArtifacts: verifiedArtifacts
        )
        do {
            _ = try await missingSelfTestHookFacade.proveOutboundToEthereum(artifactInput)
            XCTFail("Ethereum outbound prover must require the native self-test hook")
        } catch {
            XCTAssertEqual(
                error as? EvmSccpProverError,
                .invalidPublicInputs("nativeProverSelfTestFunction")
            )
        }
        XCTAssertFalse(missingSelfTestHookProverCalled)
        var driftingSelfTestHookProverCalled = false
        let driftingSelfTestHookFacade = EthereumMainnetSccp(
            proveFunction: { _ in
                driftingSelfTestHookProverCalled = true
                return Self.sampleGroth16ProofBytes()
            },
            nativeProverSelfTestFunction: { _, _, _ in
                try EthereumMainnetNativeEvmProverSelfTestSdkResult(
                    requestHash: "0x" + String(repeating: "e1", count: 32),
                    witnessHash: "0x" + String(repeating: "e2", count: 32),
                    sourceProofHash: "0x" + String(repeating: "e3", count: 32),
                    proofHash: "0x" + String(repeating: "97", count: 32),
                    publicSignalWords: (0..<9).map { index in
                        "0x" + String(repeating: String(format: "%02x", index + 0x20), count: 32)
                    },
                    calldataHash: "0x" + String(repeating: "e5", count: 32),
                    toriiSubmitPayloadHash: "0x" + String(repeating: "e6", count: 32)
                )
            },
            nativeProverArtifacts: verifiedArtifacts
        )
        do {
            _ = try await driftingSelfTestHookFacade.proveOutboundToEthereum(artifactInput)
            XCTFail("Ethereum outbound prover must reject drifting native self-test output")
        } catch {
            XCTAssertEqual(
                error as? EvmSccpProverError,
                .invalidPublicInputs("nativeProverSelfTestResult")
            )
        }
        XCTAssertFalse(driftingSelfTestHookProverCalled)
        var factoryBoundRequest: EvmSccpProofRequest?
        let factoryBoundFacade = try EthereumMainnetSccp.fromNativeProverBundle(
            proveFunction: { request in
                factoryBoundRequest = request
                return Self.sampleGroth16ProofBytes()
            },
            nativeProverSelfTestFunction: { _, expected, _ in expected },
            nativeProverBundle: verifiedBundle,
            sdk: "swift"
        ) { path in
            guard let bytes = artifactBytesByPath[path] else {
                throw EvmSccpProverError.invalidPublicInputs(path)
            }
            return bytes
        }
        let factoryBoundResult = try await factoryBoundFacade.proveOutboundToEthereum(artifactInput)
        XCTAssertEqual(factoryBoundRequest?.proofArtifactHash, proofArtifactHash)
        XCTAssertEqual(factoryBoundRequest?.provingKeyHash, provingKeyHash)
        XCTAssertEqual(factoryBoundResult.proofArtifactHash, proofArtifactHash)
        XCTAssertEqual(factoryBoundResult.provingKeyHash, provingKeyHash)
        XCTAssertThrowsError(try EthereumMainnetSccp.fromNativeProverBundle(
            nativeProverBundle: verifiedBundle,
            sdk: "swift"
        ) { path in
            if path == verifiedBundle.crossSdkFixtureParityArtifact {
                throw EvmSccpProverError.invalidPublicInputs("crossSdkFixtureParityArtifact")
            }
            return artifactBytesByPath[path] ?? Data()
        }) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("crossSdkFixtureParityArtifact"))
        }
        XCTAssertThrowsError(try EthereumMainnetSccp.fromNativeProverBundle(
            nativeProverBundle: verifiedBundle,
            sdk: "swift"
        ) { path in
            if path == verifiedBundle.nativeProverSelfTestArtifact {
                throw EvmSccpProverError.invalidPublicInputs("nativeProverSelfTestArtifact")
            }
            return artifactBytesByPath[path] ?? Data()
        }) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("nativeProverSelfTestArtifact"))
        }
        let implementationUnboundArtifacts = EthereumMainnetNativeEvmProverArtifacts(
            hashAlgorithm: sccpNativeEvmProverArtifactHashAlgorithmV1,
            nativeProverBundle: verifiedBundle,
            proofArtifactHash: proofArtifactHash,
            provingKeyHash: provingKeyHash,
            verifierKeyHash: verifierKeyHash,
            crossSdkFixtureParityHash: parityFixtureHash,
            crossSdkFixtureParity: verifiedArtifacts.crossSdkFixtureParity,
            nativeProverSelfTestHash: selfTestFixtureHash,
            nativeProverSelfTest: verifiedArtifacts.nativeProverSelfTest,
            sdk: "swift",
            implementation: "native-swift",
            implementationHash: nil
        )
        var implementationUnboundProverCalled = false
        let implementationUnboundFacade = EthereumMainnetSccp(
            proveFunction: { _ in
                implementationUnboundProverCalled = true
                return Self.sampleGroth16ProofBytes()
            },
            nativeProverArtifacts: implementationUnboundArtifacts
        )
        do {
            _ = try await implementationUnboundFacade.proveOutboundToEthereum(artifactInput)
            XCTFail("Ethereum outbound prover must reject implementation-unbound native artifacts")
        } catch {
            XCTAssertEqual(
                error as? EvmSccpProverError,
                .invalidPublicInputs("nativeProverArtifacts.implementationHash")
            )
        }
        XCTAssertFalse(implementationUnboundProverCalled)
        let paddedSdkArtifacts = EthereumMainnetNativeEvmProverArtifacts(
            hashAlgorithm: sccpNativeEvmProverArtifactHashAlgorithmV1,
            nativeProverBundle: verifiedBundle,
            proofArtifactHash: proofArtifactHash,
            provingKeyHash: provingKeyHash,
            verifierKeyHash: verifierKeyHash,
            crossSdkFixtureParityHash: parityFixtureHash,
            crossSdkFixtureParity: verifiedArtifacts.crossSdkFixtureParity,
            nativeProverSelfTestHash: selfTestFixtureHash,
            nativeProverSelfTest: verifiedArtifacts.nativeProverSelfTest,
            sdk: " swift ",
            implementation: "native-swift",
            implementationHash: implementationHash
        )
        var paddedSdkProverCalled = false
        let paddedSdkFacade = EthereumMainnetSccp(
            proveFunction: { _ in
                paddedSdkProverCalled = true
                return Self.sampleGroth16ProofBytes()
            },
            nativeProverArtifacts: paddedSdkArtifacts
        )
        do {
            _ = try await paddedSdkFacade.proveOutboundToEthereum(artifactInput)
            XCTFail("Ethereum outbound prover must reject padded native artifact sdk")
        } catch {
            XCTAssertEqual(
                error as? EvmSccpProverError,
                .invalidPublicInputs("nativeProverArtifacts.implementationHash")
            )
        }
        XCTAssertFalse(paddedSdkProverCalled)
        var paddedSelfTestHookCalled = false
        let paddedSdkSelfTestFacade = EthereumMainnetSccp(
            nativeProverSelfTestFunction: { _, expected, _ in
                paddedSelfTestHookCalled = true
                return expected
            },
            nativeProverArtifacts: paddedSdkArtifacts
        )
        do {
            _ = try await paddedSdkSelfTestFacade.runNativeProverSelfTest()
            XCTFail("Ethereum native prover self-test must reject padded native artifact sdk")
        } catch {
            XCTAssertEqual(
                error as? EvmSccpProverError,
                .invalidPublicInputs("nativeProverArtifacts.nativeProverSelfTest")
            )
        }
        XCTAssertFalse(paddedSelfTestHookCalled)
        let verifierKeyUnboundArtifacts = EthereumMainnetNativeEvmProverArtifacts(
            hashAlgorithm: sccpNativeEvmProverArtifactHashAlgorithmV1,
            nativeProverBundle: verifiedBundle,
            proofArtifactHash: proofArtifactHash,
            provingKeyHash: provingKeyHash,
            verifierKeyHash: "0x" + String(repeating: "ef", count: 32),
            crossSdkFixtureParityHash: parityFixtureHash,
            crossSdkFixtureParity: verifiedArtifacts.crossSdkFixtureParity,
            nativeProverSelfTestHash: selfTestFixtureHash,
            nativeProverSelfTest: verifiedArtifacts.nativeProverSelfTest,
            sdk: "swift",
            implementation: "native-swift",
            implementationHash: implementationHash
        )
        var verifierKeyUnboundProverCalled = false
        let verifierKeyUnboundFacade = EthereumMainnetSccp(
            proveFunction: { _ in
                verifierKeyUnboundProverCalled = true
                return Self.sampleGroth16ProofBytes()
            },
            nativeProverArtifacts: verifierKeyUnboundArtifacts
        )
        do {
            _ = try await verifierKeyUnboundFacade.proveOutboundToEthereum(artifactInput)
            XCTFail("Ethereum outbound prover must reject verifier-key-unbound native artifacts")
        } catch {
            XCTAssertEqual(
                error as? EvmSccpProverError,
                .invalidPublicInputs("nativeProverArtifacts.verifierKeyHash")
            )
        }
        XCTAssertFalse(verifierKeyUnboundProverCalled)
        let selfTestUnboundArtifacts = EthereumMainnetNativeEvmProverArtifacts(
            hashAlgorithm: sccpNativeEvmProverArtifactHashAlgorithmV1,
            nativeProverBundle: verifiedBundle,
            proofArtifactHash: proofArtifactHash,
            provingKeyHash: provingKeyHash,
            verifierKeyHash: verifierKeyHash,
            crossSdkFixtureParityHash: parityFixtureHash,
            crossSdkFixtureParity: verifiedArtifacts.crossSdkFixtureParity,
            nativeProverSelfTestHash: nil,
            nativeProverSelfTest: nil,
            sdk: "swift",
            implementation: "native-swift",
            implementationHash: implementationHash
        )
        var selfTestUnboundProverCalled = false
        let selfTestUnboundFacade = EthereumMainnetSccp(
            proveFunction: { _ in
                selfTestUnboundProverCalled = true
                return Self.sampleGroth16ProofBytes()
            },
            nativeProverArtifacts: selfTestUnboundArtifacts
        )
        do {
            _ = try await selfTestUnboundFacade.proveOutboundToEthereum(artifactInput)
            XCTFail("Ethereum outbound prover must reject self-test-unbound native artifacts")
        } catch {
            XCTAssertEqual(
                error as? EvmSccpProverError,
                .invalidPublicInputs("nativeProverArtifacts.nativeProverSelfTestHash")
            )
        }
        XCTAssertFalse(selfTestUnboundProverCalled)
        XCTAssertThrowsError(try verifiedBundle.verifiedArtifacts(
            proofArtifactBytes: Data([0]),
            provingKeyBytes: provingKeyBytes,
            verifierKeyBytes: verifierKeyBytes
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("proofArtifactBytes"))
        }
        let tinyProofArtifactBytes = Data([1, 2, 3, 4, 5, 6, 7])
        let tinyProofArtifactHash = Self.sha256Hex(tinyProofArtifactBytes)
        let draftTinyBundle = try EthereumMainnetNativeEvmProverBundle(
            proofArtifact: "artifacts/eth-mainnet/proof-artifact.bin",
            proofArtifactHash: tinyProofArtifactHash,
            provingKey: "artifacts/eth-mainnet/proving-key.bin",
            provingKeyHash: provingKeyHash,
            verifierKey: "artifacts/eth-mainnet/verifier-key.bin",
            verifierKeyHash: verifierKeyHash,
            destinationBindingHash: artifactBinding.hash,
            nativeSdkArtifacts: try sccpEthNativeEvmProverRequiredImplementationsV1
                .sorted { $0.key < $1.key }
                .enumerated()
                .map { index, entry in
                    try EthereumMainnetNativeEvmProverBundleSdkArtifact(
                        sdk: entry.key,
                        implementation: entry.value,
                        proofArtifactHash: tinyProofArtifactHash,
                        provingKeyHash: provingKeyHash,
                        implementationArtifact: "artifacts/eth-mainnet/\(entry.key)-implementation.bin",
                        implementationHash: entry.key == "swift"
                            ? implementationHash
                            : "0x" + String(
                                repeating: String(format: "%02x", index + 1),
                                count: 32
                            )
                    )
                },
            nativeProverSelfTestArtifact: "artifacts/eth-mainnet/native-prover-self-test.json",
            auditHashes: Self.sampleEthereumNativeEvmProverAuditHashes(),
            expectedDestinationBindingHash: artifactBinding.hash
        )
        let tinyParityFixtureBytes = Data(Self.sampleEthereumNativeEvmProverParityFixtureJson(
            nativeProverBundle: draftTinyBundle
        ).utf8)
        let tinySelfTestFixtureBytes = Data(Self.sampleEthereumNativeEvmProverSelfTestFixtureJson(
            nativeProverBundle: draftTinyBundle
        ).utf8)
        var tinyAuditHashes = Self.sampleEthereumNativeEvmProverAuditHashes()
        tinyAuditHashes["cross_sdk_fixture_parity"] = Self.sha256Hex(tinyParityFixtureBytes)
        tinyAuditHashes["native_prover_self_test"] = Self.sha256Hex(tinySelfTestFixtureBytes)
        let tinyBundle = try EthereumMainnetNativeEvmProverBundle(
            proofArtifact: "artifacts/eth-mainnet/proof-artifact.bin",
            proofArtifactHash: tinyProofArtifactHash,
            provingKey: "artifacts/eth-mainnet/proving-key.bin",
            provingKeyHash: provingKeyHash,
            verifierKey: "artifacts/eth-mainnet/verifier-key.bin",
            verifierKeyHash: verifierKeyHash,
            destinationBindingHash: artifactBinding.hash,
            nativeSdkArtifacts: draftTinyBundle.nativeSdkArtifacts,
            crossSdkFixtureParityArtifact: "artifacts/eth-mainnet/cross-sdk-fixture-parity.json",
            nativeProverSelfTestArtifact: "artifacts/eth-mainnet/native-prover-self-test.json",
            auditHashes: tinyAuditHashes,
            expectedDestinationBindingHash: artifactBinding.hash
        )
        XCTAssertThrowsError(try tinyBundle.verifiedArtifacts(
            proofArtifactBytes: tinyProofArtifactBytes,
            provingKeyBytes: provingKeyBytes,
            verifierKeyBytes: verifierKeyBytes,
            sdk: "swift",
            implementationBytes: implementationBytes,
            crossSdkFixtureParityBytes: tinyParityFixtureBytes,
            nativeProverSelfTestBytes: tinySelfTestFixtureBytes
        )) { error in
            XCTAssertEqual(
                error as? EvmSccpProverError,
                .invalidPublicInputs("proofArtifactBytes.minBytes")
            )
        }
        XCTAssertThrowsError(try verifiedBundle.verifiedArtifacts(
            proofArtifactBytes: proofArtifactBytes,
            provingKeyBytes: provingKeyBytes,
            verifierKeyBytes: verifierKeyBytes,
            implementationBytes: implementationBytes,
            crossSdkFixtureParityBytes: parityFixtureBytes,
            nativeProverSelfTestBytes: selfTestFixtureBytes
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("sdk"))
        }
        XCTAssertThrowsError(try verifiedBundle.verifiedArtifacts(
            proofArtifactBytes: proofArtifactBytes,
            provingKeyBytes: provingKeyBytes,
            verifierKeyBytes: verifierKeyBytes,
            sdk: " swift ",
            implementationBytes: implementationBytes,
            crossSdkFixtureParityBytes: parityFixtureBytes,
            nativeProverSelfTestBytes: selfTestFixtureBytes
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("sdk"))
        }
        XCTAssertThrowsError(try verifiedBundle.verifiedArtifacts(
            proofArtifactBytes: proofArtifactBytes,
            provingKeyBytes: provingKeyBytes,
            verifierKeyBytes: verifierKeyBytes,
            sdk: "swift",
            crossSdkFixtureParityBytes: parityFixtureBytes,
            nativeProverSelfTestBytes: selfTestFixtureBytes
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("implementationBytes"))
        }
        XCTAssertThrowsError(try verifiedBundle.verifiedArtifacts(
            proofArtifactBytes: proofArtifactBytes,
            provingKeyBytes: provingKeyBytes,
            verifierKeyBytes: verifierKeyBytes,
            sdk: "swift",
            implementationBytes: Data("tampered".utf8),
            crossSdkFixtureParityBytes: parityFixtureBytes,
            nativeProverSelfTestBytes: selfTestFixtureBytes
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("implementationBytes"))
        }
        XCTAssertThrowsError(try verifiedBundle.verifiedArtifacts(
            proofArtifactBytes: proofArtifactBytes,
            provingKeyBytes: provingKeyBytes,
            verifierKeyBytes: verifierKeyBytes,
            sdk: "swift",
            implementationBytes: implementationBytes,
            nativeProverSelfTestBytes: selfTestFixtureBytes
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("crossSdkFixtureParityBytes"))
        }
        XCTAssertThrowsError(try verifiedBundle.verifiedArtifacts(
            proofArtifactBytes: proofArtifactBytes,
            provingKeyBytes: provingKeyBytes,
            verifierKeyBytes: verifierKeyBytes,
            sdk: "swift",
            implementationBytes: implementationBytes,
            crossSdkFixtureParityBytes: Data("{}".utf8),
            nativeProverSelfTestBytes: selfTestFixtureBytes
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("crossSdkFixtureParityBytes"))
        }
        XCTAssertThrowsError(try verifiedBundle.verifiedArtifacts(
            proofArtifactBytes: proofArtifactBytes,
            provingKeyBytes: provingKeyBytes,
            verifierKeyBytes: verifierKeyBytes,
            sdk: "swift",
            implementationBytes: implementationBytes,
            crossSdkFixtureParityBytes: parityFixtureBytes
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("nativeProverSelfTestBytes"))
        }
        XCTAssertThrowsError(try verifiedBundle.verifiedArtifacts(
            proofArtifactBytes: proofArtifactBytes,
            provingKeyBytes: provingKeyBytes,
            verifierKeyBytes: verifierKeyBytes,
            sdk: "swift",
            implementationBytes: implementationBytes,
            crossSdkFixtureParityBytes: parityFixtureBytes,
            nativeProverSelfTestBytes: Data("{}".utf8)
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("nativeProverSelfTestBytes"))
        }
        let flaggedArtifactBytes = Self.nativeEvmProverArtifactBytes("proof.wasm swift artifact marker")
        let flaggedArtifactHash = Self.sha256Hex(flaggedArtifactBytes)
        let draftFlaggedBundle = try EthereumMainnetNativeEvmProverBundle(
            proofArtifact: "artifacts/eth-mainnet/proof-artifact.bin",
            proofArtifactHash: flaggedArtifactHash,
            provingKey: "artifacts/eth-mainnet/proving-key.bin",
            provingKeyHash: provingKeyHash,
            verifierKey: "artifacts/eth-mainnet/verifier-key.bin",
            verifierKeyHash: verifierKeyHash,
            destinationBindingHash: artifactBinding.hash,
            nativeSdkArtifacts: try sccpEthNativeEvmProverRequiredImplementationsV1
                .sorted { $0.key < $1.key }
                .enumerated()
                .map { index, entry in
                    try EthereumMainnetNativeEvmProverBundleSdkArtifact(
                        sdk: entry.key,
                        implementation: entry.value,
                        proofArtifactHash: flaggedArtifactHash,
                        provingKeyHash: provingKeyHash,
                        implementationArtifact: "artifacts/eth-mainnet/\(entry.key)-implementation.bin",
                        implementationHash: entry.key == "swift"
                            ? implementationHash
                            : "0x" + String(
                                repeating: String(format: "%02x", index + 1),
                                count: 32
                            )
                    )
                },
            nativeProverSelfTestArtifact: "artifacts/eth-mainnet/native-prover-self-test.json",
            auditHashes: Self.sampleEthereumNativeEvmProverAuditHashes(),
            expectedDestinationBindingHash: artifactBinding.hash
        )
        let flaggedParityFixtureBytes = Data(Self.sampleEthereumNativeEvmProverParityFixtureJson(
            nativeProverBundle: draftFlaggedBundle
        ).utf8)
        let flaggedSelfTestFixtureBytes = Data(Self.sampleEthereumNativeEvmProverSelfTestFixtureJson(
            nativeProverBundle: draftFlaggedBundle
        ).utf8)
        var flaggedAuditHashes = Self.sampleEthereumNativeEvmProverAuditHashes()
        flaggedAuditHashes["cross_sdk_fixture_parity"] = Self.sha256Hex(flaggedParityFixtureBytes)
        flaggedAuditHashes["native_prover_self_test"] = Self.sha256Hex(flaggedSelfTestFixtureBytes)
        let flaggedBundle = try EthereumMainnetNativeEvmProverBundle(
            proofArtifact: "artifacts/eth-mainnet/proof-artifact.bin",
            proofArtifactHash: flaggedArtifactHash,
            provingKey: "artifacts/eth-mainnet/proving-key.bin",
            provingKeyHash: provingKeyHash,
            verifierKey: "artifacts/eth-mainnet/verifier-key.bin",
            verifierKeyHash: verifierKeyHash,
            destinationBindingHash: artifactBinding.hash,
            nativeSdkArtifacts: draftFlaggedBundle.nativeSdkArtifacts,
            crossSdkFixtureParityArtifact: "artifacts/eth-mainnet/cross-sdk-fixture-parity.json",
            nativeProverSelfTestArtifact: "artifacts/eth-mainnet/native-prover-self-test.json",
            auditHashes: flaggedAuditHashes,
            expectedDestinationBindingHash: artifactBinding.hash
        )
        XCTAssertThrowsError(try flaggedBundle.verifiedArtifacts(
            proofArtifactBytes: flaggedArtifactBytes,
            provingKeyBytes: provingKeyBytes,
            verifierKeyBytes: verifierKeyBytes,
            crossSdkFixtureParityBytes: flaggedParityFixtureBytes,
            nativeProverSelfTestBytes: flaggedSelfTestFixtureBytes
        )) { error in
            XCTAssertEqual(
                error as? EvmSccpProverError,
                .invalidPublicInputs("proofArtifactBytes.forbiddenMarker")
            )
        }
        XCTAssertThrowsError(try Self.sampleEthereumNativeEvmProverBundle(
            destinationBindingHash: binding.hash,
            noWasm: false
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("nativeProverBundle.noWasm"))
        }
        XCTAssertThrowsError(try Self.sampleEthereumNativeEvmProverBundle(
            destinationBindingHash: "0x" + String(repeating: "95", count: 32),
            expectedDestinationBindingHash: binding.hash
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("nativeProverBundle.destinationBindingHash"))
        }

        let proofBytes = Self.sampleGroth16ProofBytes()
        let proofResult = try wrapEvmSccpProofResult(proofBytes: proofBytes, request: ethRequest)
        do {
            _ = try await facade.buildOutboundProofRequest(EvmSccpProofRequestInput(
                publicInputs: ethRequest.publicInputs,
                bundleBytes: ethRequest.bundleBytes,
                sourceProofBytes: ethRequest.sourceProofBytes,
                statementHash: ethRequest.statementHash,
                destinationBindingHash: "0x" + String(repeating: "99", count: 32),
                destinationBinding: ethRequest.destinationBinding
            ))
            XCTFail("Ethereum outbound facade must reject forged destinationBindingHash before returning request")
        } catch let error as EvmSccpProverError {
            XCTAssertEqual(error, .invalidPublicInputs("destinationBindingHash"))
        }
        let forgedBindingHashRequest = try buildEvmSccpProofRequest(EvmSccpProofRequestInput(
            publicInputs: ethRequest.publicInputs,
            bundleBytes: ethRequest.bundleBytes,
            sourceProofBytes: ethRequest.sourceProofBytes,
            statementHash: ethRequest.statementHash,
            destinationBindingHash: "0x" + String(repeating: "99", count: 32),
            destinationBinding: ethRequest.destinationBinding
        ))
        XCTAssertThrowsError(try wrapEvmSccpProofResult(
            proofBytes: proofBytes,
            request: forgedBindingHashRequest
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("destinationBindingHash"))
        }
        XCTAssertThrowsError(try facade.buildEthereumCalldata(EvmSccpSubmissionInput(proofResult: proofResult))) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("nativeProverArtifacts"))
        }
        let submissionFacade = EthereumMainnetSccp(nativeProverArtifacts: verifiedArtifacts)
        let submission = try submissionFacade.buildEthereumCalldata(
            EvmSccpSubmissionInput(proofResult: artifactBoundResult)
        )
        XCTAssertEqual(submission.targetDomain, sccpDomainEthereum)
        XCTAssertEqual(submission.proofBytes, proofBytes)
        let submitFacade = EthereumMainnetSccp(outboundSubmitFunction: { submission in
            XCTAssertEqual(submission.targetDomain, sccpDomainEthereum)
            XCTAssertEqual(submission.proofBytes, proofBytes)
            return "eth-submitted"
        }, nativeProverArtifacts: verifiedArtifacts)
        let submitted = try await submitFacade.submitOutboundToEthereum(
            EvmSccpSubmissionInput(proofResult: artifactBoundResult)
        )
        XCTAssertEqual(submitted as? String, "eth-submitted")
        var guardedSubmitterCalled = false
        let guardedSubmitFacade = EthereumMainnetSccp(
            executionProvider: EthereumMainnetExecutionProviderStub(
                chainId: "0x38",
                receipt: [:],
                block: [:]
            ),
            outboundSubmitFunction: { _ in
                guardedSubmitterCalled = true
                return "wrong-chain"
            },
            nativeProverArtifacts: verifiedArtifacts
        )
        do {
            _ = try await guardedSubmitFacade.submitOutboundToEthereum(
                EvmSccpSubmissionInput(proofResult: artifactBoundResult)
            )
            XCTFail("Ethereum outbound submitter must reject non-mainnet execution RPC")
        } catch let error as EvmSccpProverError {
            XCTAssertEqual(error, .invalidPublicInputs("eth_chainId"))
        }
        XCTAssertFalse(guardedSubmitterCalled)
        do {
            _ = try await submissionFacade.submitOutboundToEthereum(
                EvmSccpSubmissionInput(proofResult: artifactBoundResult)
            )
            XCTFail("Ethereum outbound submitter must be app-supplied")
        } catch let error as EvmSccpProverError {
            XCTAssertEqual(error, .localProverUnavailable)
        }
        XCTAssertThrowsError(try facade.buildEthereumCalldata(EvmSccpSubmissionInput(
            publicInputs: Self.sampleEvmPublicInputs(targetDomain: sccpDomainEthereum),
            proofBytes: proofBytes,
            statementHash: String(repeating: "56", count: 32),
            destinationBindingHash: binding.hash
        ))) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("proofResult.destinationBinding"))
        }

        XCTAssertThrowsError(try sccpEthereumMainnetDestinationBinding(
            verifierAddress: "0x" + String(repeating: "11", count: 20),
            bridgeAddress: "0x" + String(repeating: "22", count: 20),
            verifierCodeHash: "0x" + String(repeating: "bb", count: 32),
            verifierKeyHash: "0x" + String(repeating: "cc", count: 32),
            networkId: "0x" + String(repeating: "33", count: 32)
        )) { error in
            XCTAssertEqual(error as? SccpSourceProofHashError, .invalidSourceMaterial("networkId"))
        }

        do {
            _ = try await facade.buildOutboundProofRequest(Self.sampleEvmProofRequestInput(
                publicInputs: Self.sampleEvmPublicInputs(targetDomain: sccpDomainBsc)
            ))
            XCTFail("BSC public inputs must not use the Ethereum mainnet facade")
        } catch let error as EvmSccpProverError {
            XCTAssertEqual(error, .invalidPublicInputs("request.targetDomain"))
        }

        do {
            _ = try await facade.buildOutboundProofRequest(EvmSccpProofRequestInput(
                publicInputs: Self.sampleEvmPublicInputs(targetDomain: sccpDomainEthereum),
                bundleBytes: Data([5, 6, 7]),
                sourceProofBytes: Data([9, 10]),
                statementHash: String(repeating: "56", count: 32),
                destinationBindingHash: binding.hash,
                sourceDomain: sccpDomainBsc,
                destinationBinding: binding
            ))
            XCTFail("Ethereum outbound facade must reject non-SORA source domains")
        } catch let error as EvmSccpProverError {
            XCTAssertEqual(error, .invalidPublicInputs("sourceDomain"))
        }

        var outboundProverCalled = false
        let guardedProveFacade = EthereumMainnetSccp(proveFunction: { _ in
            outboundProverCalled = true
            return proofBytes
        })
        do {
            _ = try await guardedProveFacade.proveOutboundToEthereum(Self.sampleEvmProofRequestInput(
                publicInputs: Self.sampleEvmPublicInputs(targetDomain: sccpDomainBsc)
            ))
            XCTFail("Ethereum outbound prover callback must not see BSC requests")
        } catch let error as EvmSccpProverError {
            XCTAssertEqual(error, .invalidPublicInputs("request.targetDomain"))
        }
        XCTAssertFalse(outboundProverCalled)
    }

    func testEthereumMainnetSccpBuildsLocalAdmissionSubmission() throws {
        let input = EthereumMainnetLocalAdmissionSubmissionInput(
            proofBytes: Data([1, 2, 3]),
            publicInputsBytes: Data([4, 5, 6]),
            bundleBytes: Data([7, 8, 9]),
            envelopeBytes: Data([10, 11, 12]),
            statementHash: "0x" + String(repeating: "66", count: 32),
            sourceVerifierMaterialHash: "0x" + String(repeating: "77", count: 32),
            sourceAdapterEngineDeploymentHash: "0x" + String(repeating: "88", count: 32)
        )
        let submission = try buildEthereumMainnetSccpLocalAdmissionSubmission(input)
        let facadeSubmission = try EthereumMainnetSccp().buildLocalAdmissionSubmission(input)

        XCTAssertEqual(submission.platformPayload, sccpLocalAdmissionSubmissionKindV1)
        XCTAssertEqual(submission.envelopeEncoding, sccpLocalAdmissionEnvelopeEncodingV1)
        XCTAssertEqual(submission.verifierEntrypoint, sccpLocalAdmissionEntrypointV1)
        XCTAssertEqual(submission.sourceDomain, sccpDomainEthereum)
        XCTAssertEqual(submission.targetDomain, sccpDomainSora)
        XCTAssertTrue(submission.arguments.isEmpty)
        XCTAssertEqual(submission.proofBytes, Data([1, 2, 3]))
        XCTAssertEqual(submission.publicInputsBytes, Data([4, 5, 6]))
        XCTAssertEqual(submission.bundleBytes, Data([7, 8, 9]))
        XCTAssertEqual(submission.envelopeBytes, Data([10, 11, 12]))
        XCTAssertEqual(submission.localAdmission.proofBytes, Data([1, 2, 3]))
        XCTAssertEqual(submission.envelopeHex, facadeSubmission.envelopeHex)

        XCTAssertThrowsError(try buildEthereumMainnetSccpLocalAdmissionSubmission(
            EthereumMainnetLocalAdmissionSubmissionInput(
                proofBytes: Data([1, 2, 3]),
                publicInputsBytes: Data([4, 5, 6]),
                bundleBytes: Data([7, 8, 9]),
                envelopeBytes: Data([10, 11, 12]),
                statementHash: "0x" + String(repeating: "66", count: 32),
                sourceVerifierMaterialHash: "0x" + String(repeating: "77", count: 32),
                sourceAdapterEngineDeploymentHash: "0x" + String(repeating: "88", count: 32),
                sourceDomain: sccpDomainBsc
            )
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("ETH -> SORA"))
        }
        XCTAssertThrowsError(try buildEthereumMainnetSccpLocalAdmissionSubmission(
            EthereumMainnetLocalAdmissionSubmissionInput(
                proofBytes: Data([0, 0]),
                publicInputsBytes: Data([4, 5, 6]),
                bundleBytes: Data([7, 8, 9]),
                envelopeBytes: Data([10, 11, 12]),
                statementHash: "0x" + String(repeating: "66", count: 32),
                sourceVerifierMaterialHash: "0x" + String(repeating: "77", count: 32),
                sourceAdapterEngineDeploymentHash: "0x" + String(repeating: "88", count: 32)
            )
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .allZeroProof)
        }
        XCTAssertThrowsError(try buildEthereumMainnetSccpLocalAdmissionSubmission(
            EthereumMainnetLocalAdmissionSubmissionInput(
                proofBytes: Data([1, 2, 3]),
                publicInputsBytes: Data([0, 0]),
                bundleBytes: Data([7, 8, 9]),
                envelopeBytes: Data([10, 11, 12]),
                statementHash: "0x" + String(repeating: "66", count: 32),
                sourceVerifierMaterialHash: "0x" + String(repeating: "77", count: 32),
                sourceAdapterEngineDeploymentHash: "0x" + String(repeating: "88", count: 32)
            )
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .allZeroProof)
        }
        XCTAssertThrowsError(try buildEthereumMainnetSccpLocalAdmissionSubmission(
            EthereumMainnetLocalAdmissionSubmissionInput(
                proofBytes: Data([1, 2, 3]),
                publicInputsBytes: Data([4, 5, 6]),
                bundleBytes: Data([0, 0]),
                envelopeBytes: Data([10, 11, 12]),
                statementHash: "0x" + String(repeating: "66", count: 32),
                sourceVerifierMaterialHash: "0x" + String(repeating: "77", count: 32),
                sourceAdapterEngineDeploymentHash: "0x" + String(repeating: "88", count: 32)
            )
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .allZeroProof)
        }
        XCTAssertThrowsError(try buildEthereumMainnetSccpLocalAdmissionSubmission(
            EthereumMainnetLocalAdmissionSubmissionInput(
                proofBytes: Data([1, 2, 3]),
                publicInputsBytes: Data([4, 5, 6]),
                bundleBytes: Data([7, 8, 9]),
                envelopeBytes: Data(),
                statementHash: "0x" + String(repeating: "66", count: 32),
                sourceVerifierMaterialHash: "0x" + String(repeating: "77", count: 32),
                sourceAdapterEngineDeploymentHash: "0x" + String(repeating: "88", count: 32)
            )
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .emptyProof)
        }
        XCTAssertThrowsError(try buildEthereumMainnetSccpLocalAdmissionSubmission(
            EthereumMainnetLocalAdmissionSubmissionInput(
                proofBytes: Data([1, 2, 3]),
                publicInputsBytes: Data([4, 5, 6]),
                bundleBytes: Data([7, 8, 9]),
                envelopeBytes: Data([0, 0]),
                statementHash: "0x" + String(repeating: "66", count: 32),
                sourceVerifierMaterialHash: "0x" + String(repeating: "77", count: 32),
                sourceAdapterEngineDeploymentHash: "0x" + String(repeating: "88", count: 32)
            )
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .allZeroProof)
        }
        XCTAssertThrowsError(try buildEthereumMainnetSccpLocalAdmissionSubmission(
            EthereumMainnetLocalAdmissionSubmissionInput(
                proofBytes: Data([1, 2, 3]),
                publicInputsBytes: Data([4, 5, 6]),
                bundleBytes: Data([7, 8, 9]),
                envelopeBytes: Data([10, 11, 12]),
                statementHash: "0x" + String(repeating: "00", count: 32),
                sourceVerifierMaterialHash: "0x" + String(repeating: "77", count: 32),
                sourceAdapterEngineDeploymentHash: "0x" + String(repeating: "88", count: 32)
            )
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .zeroField("statementHash"))
        }
        XCTAssertThrowsError(try buildEthereumMainnetSccpLocalAdmissionSubmission(
            EthereumMainnetLocalAdmissionSubmissionInput(
                proofBytes: Data([1, 2, 3]),
                publicInputsBytes: Data([4, 5, 6]),
                bundleBytes: Data([7, 8, 9]),
                envelopeBytes: Data([10, 11, 12]),
                statementHash: "0x" + String(repeating: "66", count: 32),
                sourceVerifierMaterialHash: "0x" + String(repeating: "00", count: 32),
                sourceAdapterEngineDeploymentHash: "0x" + String(repeating: "88", count: 32)
            )
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .zeroField("sourceVerifierMaterialHash"))
        }
        XCTAssertThrowsError(try buildEthereumMainnetSccpLocalAdmissionSubmission(
            EthereumMainnetLocalAdmissionSubmissionInput(
                proofBytes: Data([1, 2, 3]),
                publicInputsBytes: Data([4, 5, 6]),
                bundleBytes: Data([7, 8, 9]),
                envelopeBytes: Data([10, 11, 12]),
                statementHash: "0x" + String(repeating: "66", count: 32),
                sourceVerifierMaterialHash: "0x" + String(repeating: "77", count: 32),
                sourceAdapterEngineDeploymentHash: "0x" + String(repeating: "00", count: 32)
            )
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .zeroField("sourceAdapterEngineDeploymentHash"))
        }
        XCTAssertThrowsError(try buildEthereumMainnetSccpLocalAdmissionSubmission(
            EthereumMainnetLocalAdmissionSubmissionInput(
                proofBytes: Data([1, 2, 3]),
                publicInputsBytes: Data([4, 5, 6]),
                bundleBytes: Data([7, 8, 9]),
                envelopeBytes: Data([10, 11, 12]),
                statementHash: "0x" + String(repeating: "66", count: 32),
                sourceVerifierMaterialHash: "0x" + String(repeating: "77", count: 32),
                sourceAdapterEngineDeploymentHash: "0x" + String(repeating: "88", count: 32),
                envelopeEncoding: "abi_tuple_v1"
            )
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("localAdmission.metadata"))
        }
        XCTAssertThrowsError(try buildEthereumMainnetSccpLocalAdmissionSubmission(
            EthereumMainnetLocalAdmissionSubmissionInput(
                proofBytes: Data([1, 2, 3]),
                publicInputsBytes: Data([4, 5, 6]),
                bundleBytes: Data([7, 8, 9]),
                envelopeBytes: Data([10, 11, 12]),
                statementHash: "0x" + String(repeating: "66", count: 32),
                sourceVerifierMaterialHash: "0x" + String(repeating: "77", count: 32),
                sourceAdapterEngineDeploymentHash: "0x" + String(repeating: "88", count: 32),
                proofFamily: "debug-proof-family"
            )
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("localAdmission.metadata"))
        }
    }

    func testEthereumMainnetBeaconRestConsensusProviderCollectsFinalizedTargetEvidence() async throws {
        let finalizedHeaderRoot = "0xbb44a971e8c280f585ba430bfabfe87d9c59adf38bf9f77266b69687a148048c"
        let targetHeaderRoot = finalizedHeaderRoot
        let blockHash = "0x" + String(repeating: "bb", count: 32)
        let receiptsRoot = "0x" + String(repeating: "cc", count: 32)
        let syncCommitteePayload = try Self.sampleEthereumSyncCommitteePayload()
        let nextSyncPayload = syncCommitteePayload
        XCTAssertEqual(nextSyncPayload.count, 81_925)
        XCTAssertEqual(Self.ethereumSyncCommitteeSignersBitmap(342).count, 64)
        let syncCommitteeRoot = try ethSyncCommitteeHashFromPayload(payload: syncCommitteePayload)
        let transport = EthereumMainnetBeaconRestTransportStub(responses: [
            "https://beacon.example/eth/v1/beacon/headers/finalized?token=rpc": Self.ethereumBeaconResponse(
                Self.ethereumBeaconHeaderJson(finalizedHeaderRoot: finalizedHeaderRoot, slot: "64")
            ),
            "https://beacon.example/eth/v1/beacon/headers/64?token=rpc": Self.ethereumBeaconResponse(
                Self.ethereumBeaconHeaderJson(finalizedHeaderRoot: targetHeaderRoot, slot: "64")
            ),
            "https://beacon.example/eth/v1/beacon/blocks/64/root?token=rpc": Self.ethereumBeaconResponse(
                Self.ethereumBeaconBlockRootJson(finalizedHeaderRoot: targetHeaderRoot)
            ),
            "https://beacon.example/eth/v2/beacon/blocks/64?token=rpc": Self.ethereumBeaconResponse(
                Self.ethereumBeaconBlockJson(
                    slot: "64",
                    blockHash: blockHash,
                    receiptsRoot: receiptsRoot
                )
            ),
            "https://beacon.example/eth/v1/beacon/states/finalized/finality_checkpoints?token=rpc": Self.ethereumBeaconResponse(
                Self.ethereumBeaconCheckpointJson(finalizedHeaderRoot: finalizedHeaderRoot)
            ),
            "https://beacon.example/eth/v1/beacon/light_client/finality_update?token=rpc": Self.ethereumBeaconResponse(
                Self.ethereumBeaconFinalityUpdateJson(slot: "64", signatureSlot: "65")
            ),
        ])
        let provider = try EthereumMainnetBeaconRestConsensusProvider(
            endpoint: "https://beacon.example/eth/v1?token=rpc",
            syncCommitteeRoot: syncCommitteeRoot,
            syncCommitteePayload: syncCommitteePayload,
            headers: ["Authorization": "Bearer rpc"],
            transport: transport
        )
        let finality = try await provider.collectFinalityEvidence(
            receipt: nil,
            block: [
                "hash": blockHash,
                "number": "0x1234",
                "receiptsRoot": receiptsRoot,
                "beaconSlot": "64",
            ],
            transactionHash: nil
        )

        XCTAssertEqual(finality["executionBlockNumber"] as? String, "4660")
        XCTAssertEqual(finality["executionBlockHash"] as? String, blockHash)
        XCTAssertEqual(finality["executionReceiptsRoot"] as? String, receiptsRoot)
        XCTAssertEqual(finality["finalizedHeaderRoot"] as? String, finalizedHeaderRoot)
        XCTAssertEqual(finality["syncCommitteeRoot"] as? String, syncCommitteeRoot)
        XCTAssertEqual(finality["beaconSlot"] as? String, "64")
        XCTAssertEqual(finality["finalityBranch"] as? [String], Self.ethereumFinalityBranch)
        XCTAssertEqual(finality["syncCommitteeBits"] as? String, Self.ethereumSyncCommitteeSupermajorityBits)
        XCTAssertEqual(finality["syncCommitteeSignature"] as? String, "0x" + String(repeating: "34", count: 96))
        XCTAssertEqual(finality["syncCommitteeParticipation"] as? String, Self.ethereumSyncCommitteeSupermajorityParticipation)
        XCTAssertEqual(finality["syncSignatureSlot"] as? String, "65")
        XCTAssertEqual(transport.calls.map { $0.url }, [
            "https://beacon.example/eth/v1/beacon/headers/finalized?token=rpc",
            "https://beacon.example/eth/v1/beacon/headers/64?token=rpc",
            "https://beacon.example/eth/v1/beacon/blocks/64/root?token=rpc",
            "https://beacon.example/eth/v2/beacon/blocks/64?token=rpc",
            "https://beacon.example/eth/v1/beacon/states/finalized/finality_checkpoints?token=rpc",
            "https://beacon.example/eth/v1/beacon/light_client/finality_update?token=rpc",
        ])
        XCTAssertEqual(transport.calls.map { $0.headers["Authorization"] }, ["Bearer rpc", "Bearer rpc", "Bearer rpc", "Bearer rpc", "Bearer rpc", "Bearer rpc"])
    }

    func testEthereumMainnetBeaconRestConsensusProviderDerivesTargetSlotFromTimestamp() async throws {
        let finalizedHeaderRoot = "0xbb44a971e8c280f585ba430bfabfe87d9c59adf38bf9f77266b69687a148048c"
        let targetHeaderRoot = finalizedHeaderRoot
        let blockHash = "0x" + String(repeating: "bb", count: 32)
        let receiptsRoot = "0x" + String(repeating: "cc", count: 32)
        let transport = EthereumMainnetBeaconRestTransportStub(responses: [
            "https://beacon.example/eth/v1/beacon/genesis": Self.ethereumBeaconResponse(
                Self.ethereumBeaconGenesisJson(genesisTime: "100")
            ),
            "https://beacon.example/eth/v1/beacon/headers/finalized": Self.ethereumBeaconResponse(
                Self.ethereumBeaconHeaderJson(finalizedHeaderRoot: finalizedHeaderRoot, slot: "64")
            ),
            "https://beacon.example/eth/v1/beacon/headers/64": Self.ethereumBeaconResponse(
                Self.ethereumBeaconHeaderJson(finalizedHeaderRoot: targetHeaderRoot, slot: "64")
            ),
            "https://beacon.example/eth/v1/beacon/blocks/64/root": Self.ethereumBeaconResponse(
                Self.ethereumBeaconBlockRootJson(finalizedHeaderRoot: targetHeaderRoot)
            ),
            "https://beacon.example/eth/v2/beacon/blocks/64": Self.ethereumBeaconResponse(
                Self.ethereumBeaconBlockJson(
                    slot: "64",
                    blockHash: blockHash,
                    receiptsRoot: receiptsRoot
                )
            ),
            "https://beacon.example/eth/v1/beacon/states/finalized/finality_checkpoints": Self.ethereumBeaconResponse(
                Self.ethereumBeaconCheckpointJson(finalizedHeaderRoot: finalizedHeaderRoot)
            ),
            "https://beacon.example/eth/v1/beacon/light_client/finality_update": Self.ethereumBeaconResponse(
                Self.ethereumBeaconFinalityUpdateJson(slot: "64", signatureSlot: "65")
            ),
        ])
        let provider = try EthereumMainnetBeaconRestConsensusProvider(
            endpoint: "https://beacon.example/eth/v1",
            syncCommitteeRoot: "0x" + String(repeating: "aa", count: 32),
            transport: transport
        )

        let finality = try await provider.collectFinalityEvidence(
            receipt: nil,
            block: [
                "hash": blockHash,
                "number": "0x1234",
                "receiptsRoot": receiptsRoot,
                "timestamp": "0x364",
            ],
            transactionHash: nil
        )

        XCTAssertEqual(finality["finalizedHeaderRoot"] as? String, finalizedHeaderRoot)
        XCTAssertEqual(finality["beaconSlot"] as? String, "64")
        XCTAssertEqual(transport.calls.map { $0.url }, [
            "https://beacon.example/eth/v1/beacon/genesis",
            "https://beacon.example/eth/v1/beacon/headers/finalized",
            "https://beacon.example/eth/v1/beacon/headers/64",
            "https://beacon.example/eth/v1/beacon/blocks/64/root",
            "https://beacon.example/eth/v2/beacon/blocks/64",
            "https://beacon.example/eth/v1/beacon/states/finalized/finality_checkpoints",
            "https://beacon.example/eth/v1/beacon/light_client/finality_update",
        ])
    }

    func testEthereumMainnetBeaconRestURLSessionTransportRejectsOversizedBodies() async throws {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [EthereumMainnetBeaconRestURLProtocol.self]
        let session = URLSession(configuration: configuration)
        defer {
            EthereumMainnetBeaconRestURLProtocol.response = nil
            session.invalidateAndCancel()
        }
        EthereumMainnetBeaconRestURLProtocol.response = (
            statusCode: 200,
            headers: ["Content-Length": String(1024 * 1024 + 1)],
            body: Data(repeating: 0x7b, count: 1024 * 1024 + 1)
        )
        let transport = EthereumMainnetBeaconRestURLSessionTransport(session: session)
        do {
            _ = try await transport.get(
                url: URL(string: "https://beacon.example/oversized")!,
                headers: ["Authorization": "Bearer rpc"]
            )
            XCTFail("expected oversized Beacon REST response to fail")
        } catch {
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("beaconRest.response"))
        }
    }

    func testEthereumMainnetBeaconRestURLSessionTransportRejectsOversizedBodiesWithoutContentLength() async throws {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [EthereumMainnetBeaconRestURLProtocol.self]
        let session = URLSession(configuration: configuration)
        defer {
            EthereumMainnetBeaconRestURLProtocol.response = nil
            session.invalidateAndCancel()
        }
        EthereumMainnetBeaconRestURLProtocol.response = (
            statusCode: 200,
            headers: [:],
            body: Data(repeating: 0x7b, count: 1024 * 1024 + 1)
        )
        let transport = EthereumMainnetBeaconRestURLSessionTransport(session: session)
        do {
            _ = try await transport.get(
                url: URL(string: "https://beacon.example/oversized")!,
                headers: ["Authorization": "Bearer rpc"]
            )
            XCTFail("expected oversized Beacon REST response to fail")
        } catch {
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("beaconRest.response"))
        }
    }

    func testEthereumMainnetBeaconRestConsensusProviderRejectsUnsafeFinality() async throws {
        func assertEvmError(
            _ expected: EvmSccpProverError,
            _ operation: () async throws -> Void
        ) async {
            do {
                try await operation()
                XCTFail("expected \(expected)")
            } catch {
                XCTAssertEqual(error as? EvmSccpProverError, expected)
            }
        }

        let finalizedHeaderRoot = "0xed5b18104f470370f9f7ce3e5c2f4892ab541f2991e626578b76cf34819def1b"
        let block: [String: Any] = [
            "hash": "0x" + String(repeating: "bb", count: 32),
            "number": "0x1234",
            "receiptsRoot": "0x" + String(repeating: "cc", count: 32),
        ]
        let syncCommitteePayload = try Self.sampleEthereumSyncCommitteePayload()
        let syncCommitteeRoot = try ethSyncCommitteeHashFromPayload(payload: syncCommitteePayload)

        func provider(
            header: Data,
            finalizedBlockRoot: Data = Self.ethereumBeaconBlockRootJson(finalizedHeaderRoot: finalizedHeaderRoot),
            finalizedBlock: Data = Self.ethereumBeaconBlockJson(),
            checkpoint: Data = Self.ethereumBeaconCheckpointJson(finalizedHeaderRoot: finalizedHeaderRoot),
            finalityUpdate: Data = Self.ethereumBeaconFinalityUpdateJson(),
            statusCode: Int = 200
        ) throws -> EthereumMainnetBeaconRestConsensusProvider {
            let transport = EthereumMainnetBeaconRestTransportStub(responses: [
                "https://beacon.example/eth/v1/beacon/headers/finalized": Self.ethereumBeaconResponse(
                    header,
                    statusCode: statusCode
                ),
                "https://beacon.example/eth/v1/beacon/blocks/finalized/root": Self.ethereumBeaconResponse(
                    finalizedBlockRoot
                ),
                "https://beacon.example/eth/v2/beacon/blocks/finalized": Self.ethereumBeaconResponse(
                    finalizedBlock
                ),
                "https://beacon.example/eth/v1/beacon/states/finalized/finality_checkpoints": Self.ethereumBeaconResponse(
                    checkpoint
                ),
                "https://beacon.example/eth/v1/beacon/light_client/finality_update": Self.ethereumBeaconResponse(
                    finalityUpdate
                ),
            ])
            return try EthereumMainnetBeaconRestConsensusProvider(
                endpoint: "https://beacon.example/eth/v1",
                syncCommitteeRoot: syncCommitteeRoot,
                transport: transport
            )
        }

        func malformedHeader(replacing needle: String, with replacement: String) -> Data {
            let json = String(decoding: Self.ethereumBeaconHeaderJson(), as: UTF8.self)
            return Data(json.replacingOccurrences(of: needle, with: replacement).utf8)
        }

        await assertEvmError(.invalidPublicInputs("beaconRest.block")) {
            _ = try await provider(header: Self.ethereumBeaconHeaderJson()).collectFinalityEvidence(
                receipt: nil,
                block: nil,
                transactionHash: nil
            )
        }
        await assertEvmError(.invalidPublicInputs("beaconRest.response")) {
            _ = try await provider(
                header: Data("{}".utf8),
                statusCode: 503
            ).collectFinalityEvidence(receipt: nil, block: block, transactionHash: nil)
        }
        await assertEvmError(.invalidPublicInputs("beaconRest.response")) {
            _ = try await provider(
                header: Data(repeating: 0x7b, count: 1024 * 1024 + 1)
            ).collectFinalityEvidence(receipt: nil, block: block, transactionHash: nil)
        }
        var historicalBlock = block
        historicalBlock["beaconSlot"] = "32"
        let historicalTransport = EthereumMainnetBeaconRestTransportStub(responses: [
            "https://beacon.example/eth/v1/beacon/headers/finalized": Self.ethereumBeaconResponse(
                Self.ethereumBeaconHeaderJson(finalizedHeaderRoot: finalizedHeaderRoot, slot: "64")
            ),
            "https://beacon.example/eth/v1/beacon/headers/32": Self.ethereumBeaconResponse(
                Self.ethereumBeaconHeaderJson(
                    finalizedHeaderRoot: "0x" + String(repeating: "aa", count: 32),
                    slot: "32"
                )
            ),
        ])
        let historicalProvider = try EthereumMainnetBeaconRestConsensusProvider(
            endpoint: "https://beacon.example/eth/v1",
            syncCommitteeRoot: syncCommitteeRoot,
            transport: historicalTransport
        )
        await assertEvmError(.invalidPublicInputs("beaconRest.targetHeader.ancestryProof")) {
            _ = try await historicalProvider.collectFinalityEvidence(
                receipt: nil,
                block: historicalBlock,
                transactionHash: nil
            )
        }
        await assertEvmError(.invalidPublicInputs("beaconRest.finalizedHeader")) {
            _ = try await provider(
                header: Self.ethereumBeaconHeaderJson(executionOptimistic: true)
            ).collectFinalityEvidence(receipt: nil, block: block, transactionHash: nil)
        }
        await assertEvmError(
            .invalidPublicInputs("Ethereum mainnet Beacon REST finalized header.execution_optimistic")
        ) {
            _ = try await provider(
                header: malformedHeader(
                    replacing: "\"execution_optimistic\": false",
                    with: "\"execution_optimistic\": \"false\""
                )
            ).collectFinalityEvidence(receipt: nil, block: block, transactionHash: nil)
        }
        await assertEvmError(
            .invalidPublicInputs("Ethereum mainnet Beacon REST finalized header.finalized")
        ) {
            _ = try await provider(
                header: malformedHeader(
                    replacing: "\"finalized\": true",
                    with: "\"finalized\": \"true\""
                )
            ).collectFinalityEvidence(receipt: nil, block: block, transactionHash: nil)
        }
        await assertEvmError(
            .invalidPublicInputs("Ethereum mainnet Beacon REST finalized header.canonical")
        ) {
            _ = try await provider(
                header: malformedHeader(
                    replacing: "\"canonical\": true",
                    with: "\"canonical\": \"true\""
                )
            ).collectFinalityEvidence(receipt: nil, block: block, transactionHash: nil)
        }
        for (field, goodValue) in [
            ("parent_root", String(repeating: "01", count: 32)),
            ("state_root", String(repeating: "02", count: 32)),
            ("body_root", String(repeating: "03", count: 32)),
        ] {
            await assertEvmError(
                .invalidPublicInputs("Ethereum mainnet Beacon REST finalized header.data.header.message.\(field)")
            ) {
                _ = try await provider(
                    header: malformedHeader(
                        replacing: "\"\(field)\": \"0x\(goodValue)\"",
                        with: "\"\(field)\": \"0x\""
                    )
                ).collectFinalityEvidence(receipt: nil, block: block, transactionHash: nil)
            }
        }
        await assertEvmError(
            .invalidPublicInputs("Ethereum mainnet Beacon REST finalized header.data.header.signature")
        ) {
            _ = try await provider(
                header: malformedHeader(
                    replacing: "\"signature\": \"0x\(String(repeating: "12", count: 96))\"",
                    with: "\"signature\": \"0x\(String(repeating: "12", count: 95))\""
                )
            ).collectFinalityEvidence(receipt: nil, block: block, transactionHash: nil)
        }
        await assertEvmError(.invalidPublicInputs("beaconRest.finalizedBlockRoot")) {
            _ = try await provider(
                header: Self.ethereumBeaconHeaderJson(),
                finalizedBlockRoot: Self.ethereumBeaconBlockRootJson(
                    finalizedHeaderRoot: "0x" + String(repeating: "99", count: 32)
                )
            ).collectFinalityEvidence(receipt: nil, block: block, transactionHash: nil)
        }
        await assertEvmError(.invalidPublicInputs("beaconRest.executionPayload.slot")) {
            _ = try await provider(
                header: Self.ethereumBeaconHeaderJson(),
                finalizedBlock: Self.ethereumBeaconBlockJson(slot: "33")
            ).collectFinalityEvidence(receipt: nil, block: block, transactionHash: nil)
        }
        await assertEvmError(.invalidPublicInputs("beaconRest.executionPayload.blockHash")) {
            _ = try await provider(
                header: Self.ethereumBeaconHeaderJson(),
                finalizedBlock: Self.ethereumBeaconBlockJson(blockHash: "0x" + String(repeating: "99", count: 32))
            ).collectFinalityEvidence(receipt: nil, block: block, transactionHash: nil)
        }
        await assertEvmError(.invalidPublicInputs("beaconRest.executionPayload.blockNumber")) {
            _ = try await provider(
                header: Self.ethereumBeaconHeaderJson(),
                finalizedBlock: Self.ethereumBeaconBlockJson(blockNumber: "4661")
            ).collectFinalityEvidence(receipt: nil, block: block, transactionHash: nil)
        }
        await assertEvmError(.invalidPublicInputs("beaconRest.executionPayload.receiptsRoot")) {
            _ = try await provider(
                header: Self.ethereumBeaconHeaderJson(),
                finalizedBlock: Self.ethereumBeaconBlockJson(receiptsRoot: "0x" + String(repeating: "99", count: 32))
            ).collectFinalityEvidence(receipt: nil, block: block, transactionHash: nil)
        }
        await assertEvmError(.invalidPublicInputs("beaconRest.finalizedHeader")) {
            _ = try await provider(
                header: Self.ethereumBeaconHeaderJson(finalized: false)
            ).collectFinalityEvidence(receipt: nil, block: block, transactionHash: nil)
        }
        await assertEvmError(.invalidPublicInputs("beaconRest.finalizedHeader")) {
            _ = try await provider(
                header: Self.ethereumBeaconHeaderJson(canonical: false)
            ).collectFinalityEvidence(receipt: nil, block: block, transactionHash: nil)
        }
        await assertEvmError(.invalidPublicInputs("beaconRest.finalityCheckpoint")) {
            _ = try await provider(
                header: Self.ethereumBeaconHeaderJson(finalizedHeaderRoot: finalizedHeaderRoot),
                checkpoint: Self.ethereumBeaconCheckpointJson(
                    finalizedHeaderRoot: "0x" + String(repeating: "99", count: 32)
                )
            ).collectFinalityEvidence(receipt: nil, block: block, transactionHash: nil)
        }
        await assertEvmError(
            .zeroField("Ethereum mainnet Beacon REST light-client finality update.data.sync_aggregate.sync_committee_bits")
        ) {
            _ = try await provider(
                header: Self.ethereumBeaconHeaderJson(finalizedHeaderRoot: finalizedHeaderRoot),
                finalityUpdate: Self.ethereumBeaconFinalityUpdateJson(
                    syncCommitteeBits: "0x" + String(repeating: "00", count: 64)
                )
            ).collectFinalityEvidence(receipt: nil, block: block, transactionHash: nil)
        }
        await assertEvmError(
            .invalidPublicInputs("Ethereum mainnet Beacon REST light-client finality update.data.sync_aggregate.sync_committee_bits")
        ) {
            _ = try await provider(
                header: Self.ethereumBeaconHeaderJson(finalizedHeaderRoot: finalizedHeaderRoot),
                finalityUpdate: Self.ethereumBeaconFinalityUpdateJson(
                    syncCommitteeBits: "0x01" + String(repeating: "00", count: 63)
                )
            ).collectFinalityEvidence(receipt: nil, block: block, transactionHash: nil)
        }
        await assertEvmError(
            .invalidPublicInputs("Ethereum mainnet Beacon REST light-client finality update.data.finality_branch")
        ) {
            _ = try await provider(
                header: Self.ethereumBeaconHeaderJson(finalizedHeaderRoot: finalizedHeaderRoot),
                finalityUpdate: Self.ethereumBeaconFinalityUpdateJson(includeFinalityBranch: false)
            ).collectFinalityEvidence(receipt: nil, block: block, transactionHash: nil)
        }
        await assertEvmError(
            .invalidPublicInputs("Ethereum mainnet Beacon REST light-client finality update.data.finality_branch")
        ) {
            _ = try await provider(
                header: Self.ethereumBeaconHeaderJson(finalizedHeaderRoot: finalizedHeaderRoot),
                finalityUpdate: Self.ethereumBeaconFinalityUpdateJson(
                    finalityBranch: Array(Self.ethereumFinalityBranch.prefix(5))
                )
            ).collectFinalityEvidence(receipt: nil, block: block, transactionHash: nil)
        }
        await assertEvmError(
            .zeroField("Ethereum mainnet Beacon REST light-client finality update.data.sync_aggregate.sync_committee_signature")
        ) {
            _ = try await provider(
                header: Self.ethereumBeaconHeaderJson(finalizedHeaderRoot: finalizedHeaderRoot),
                finalityUpdate: Self.ethereumBeaconFinalityUpdateJson(
                    syncCommitteeSignature: "0x" + String(repeating: "00", count: 96)
                )
            ).collectFinalityEvidence(receipt: nil, block: block, transactionHash: nil)
        }
        XCTAssertThrowsError(
            try EthereumMainnetBeaconRestConsensusProvider(
                endpoint: "https://beacon.example/eth/v1",
                transport: EthereumMainnetBeaconRestTransportStub(responses: [:])
            )
        ) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("syncCommitteeRoot"))
        }
        XCTAssertThrowsError(
            try EthereumMainnetBeaconRestConsensusProvider(
                endpoint: "https://beacon.example/eth/v1",
                syncCommitteeRoot: "0x" + String(repeating: "99", count: 32),
                syncCommitteePayload: syncCommitteePayload,
                transport: EthereumMainnetBeaconRestTransportStub(responses: [:])
            )
        ) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("syncCommitteePayload"))
        }
    }

    func testEthereumReceiptTrieProofBuilderUsesRlpTransactionIndexKeys() throws {
        let blockHash = "0x" + String(repeating: "bb", count: 32)
        let logsBloom = "0x" + String(repeating: "00", count: 256)
        let typedReceipt: [String: Any] = [
            "type": "0x2",
            "transactionHash": "0x" + String(repeating: "aa", count: 32),
            "blockHash": blockHash,
            "blockNumber": "0x1234",
            "transactionIndex": "0x0",
            "status": "0x1",
            "cumulativeGasUsed": "0x5208",
            "logsBloom": logsBloom,
            "logs": [[String: Any]](),
        ]
        let legacyReceipt: [String: Any] = [
            "transactionHash": "0x" + String(repeating: "12", count: 32),
            "blockHash": blockHash,
            "blockNumber": "0x1234",
            "transactionIndex": "0x1",
            "status": "0x1",
            "cumulativeGasUsed": "0x5300",
            "logsBloom": logsBloom,
            "logs": [[String: Any]](),
        ]

        XCTAssertEqual(try evmReceiptTrieKey(0), "0x80")
        XCTAssertEqual(try evmReceiptTrieKey("0x1"), "0x01")
        XCTAssertEqual(try evmReceiptTrieKey("0x80"), "0x8180")
        XCTAssertThrowsError(try evmReceiptTrieKey("0x01")) { error in
            XCTAssertEqual(error as? SccpSourceProofHashError, .invalidRlp("transactionIndex"))
        }
        let typedReceiptRlp = try canonicalEvmReceiptRlp(typedReceipt)
        XCTAssertEqual(typedReceiptRlp.first, 0x02)

        let proof = try buildEvmReceiptTrieProofFromReceipts(
            [typedReceipt, legacyReceipt],
            transactionIndex: "0x0"
        )
        XCTAssertEqual(proof.receiptTrieKey, "0x80")
        XCTAssertEqual(proof.receiptRlp, "0x" + typedReceiptRlp.hexEncodedString())
        XCTAssertEqual(proof.receiptsRoot.count, 66)
        XCTAssertFalse(proof.receiptTrieProofNodes.isEmpty)

        let secondProof = try buildEvmReceiptTrieProofFromReceipts(
            [typedReceipt, legacyReceipt],
            transactionIndex: "0x1"
        )
        XCTAssertEqual(secondProof.receiptTrieKey, "0x01")
        var zeroTopicReceipt = legacyReceipt
        zeroTopicReceipt["logs"] = [[
            "address": "0x" + String(repeating: "12", count: 20),
            "topics": ["0x" + String(repeating: "00", count: 32)],
            "data": "0x",
        ]]
        let zeroTopicProof = try buildEvmReceiptTrieProofFromReceipts(
            [typedReceipt, zeroTopicReceipt],
            transactionIndex: "0x0"
        )
        XCTAssertEqual(zeroTopicProof.receiptRlp, "0x" + typedReceiptRlp.hexEncodedString())
        var zeroAddressReceipt = legacyReceipt
        zeroAddressReceipt["logs"] = [[
            "address": "0x" + String(repeating: "00", count: 20),
            "topics": ["0x" + String(repeating: "44", count: 32)],
            "data": "0x",
        ]]
        let zeroAddressProof = try buildEvmReceiptTrieProofFromReceipts(
            [typedReceipt, zeroAddressReceipt],
            transactionIndex: "0x0"
        )
        XCTAssertEqual(zeroAddressProof.receiptRlp, "0x" + typedReceiptRlp.hexEncodedString())

        var wrongReceiptIndex = typedReceipt
        wrongReceiptIndex["transactionIndex"] = "0x1"
        XCTAssertThrowsError(
            try buildEvmReceiptTrieProofFromReceipts([wrongReceiptIndex], transactionIndex: "0x0")
        ) { error in
            XCTAssertEqual(
                error as? SccpSourceProofHashError,
                .invalidRlp("blockReceipts[0].transactionIndex")
            )
        }
        var conflictingReceiptIndex = typedReceipt
        conflictingReceiptIndex["transaction_index"] = "0x0"
        XCTAssertThrowsError(
            try buildEvmReceiptTrieProofFromReceipts([conflictingReceiptIndex], transactionIndex: "0x0")
        ) { error in
            XCTAssertEqual(
                error as? SccpSourceProofHashError,
                .invalidRlp("blockReceipts[0].transactionIndex")
            )
        }
        var conflictingReceiptHash = typedReceipt
        conflictingReceiptHash["transaction_hash"] = typedReceipt["transactionHash"]
        XCTAssertThrowsError(
            try buildEvmReceiptTrieProofFromReceipts([conflictingReceiptHash], transactionIndex: "0x0")
        ) { error in
            XCTAssertEqual(
                error as? SccpSourceProofHashError,
                .invalidRlp("blockReceipts[0].transactionHash")
            )
        }
        var conflictingCumulativeGas = typedReceipt
        conflictingCumulativeGas["cumulative_gas_used"] = "0x5208"
        XCTAssertThrowsError(
            try buildEvmReceiptTrieProofFromReceipts([conflictingCumulativeGas], transactionIndex: "0x0")
        ) { error in
            XCTAssertEqual(
                error as? SccpSourceProofHashError,
                .invalidRlp("receipt.cumulativeGasUsed")
            )
        }
        var conflictingLogsBloom = typedReceipt
        conflictingLogsBloom["logs_bloom"] = logsBloom
        XCTAssertThrowsError(
            try buildEvmReceiptTrieProofFromReceipts([conflictingLogsBloom], transactionIndex: "0x0")
        ) { error in
            XCTAssertEqual(
                error as? SccpSourceProofHashError,
                .invalidRlp("receipt.logsBloom")
            )
        }
        var duplicateHashReceipt = legacyReceipt
        duplicateHashReceipt["transactionHash"] = typedReceipt["transactionHash"]
        XCTAssertThrowsError(
            try buildEvmReceiptTrieProofFromReceipts(
                [typedReceipt, duplicateHashReceipt],
                transactionIndex: "0x0"
            )
        ) { error in
            XCTAssertEqual(
                error as? SccpSourceProofHashError,
                .invalidRlp("blockReceipts.transactionHash")
            )
        }
        XCTAssertThrowsError(
            try buildEvmReceiptTrieProofFromReceipts([typedReceipt], transactionIndex: "0x1")
        ) { error in
            XCTAssertEqual(error as? SccpSourceProofHashError, .invalidRlp("transactionIndex"))
        }
        XCTAssertThrowsError(
            try buildEvmReceiptTrieProofFromReceipts([], transactionIndex: "0x0")
        ) { error in
            XCTAssertEqual(error as? SccpSourceProofHashError, .invalidValidatorSet("blockReceipts"))
        }
        XCTAssertThrowsError(
            try buildEvmReceiptTrieProofFromReceipts(
                Array(repeating: typedReceipt, count: 4_097),
                transactionIndex: "0x0"
            )
        ) { error in
            XCTAssertEqual(error as? SccpSourceProofHashError, .invalidValidatorSet("blockReceipts"))
        }

        var uppercaseBloomReceipt = typedReceipt
        uppercaseBloomReceipt["logsBloom"] = "0x" + String(repeating: "AA", count: 256)
        XCTAssertThrowsError(try canonicalEvmReceiptRlp(uppercaseBloomReceipt)) { error in
            XCTAssertEqual(error as? SccpSourceProofHashError, .invalidRlp("receipt.logsBloom"))
        }
        var badTypedReceipt = typedReceipt
        badTypedReceipt["type"] = "0x80"
        XCTAssertThrowsError(try canonicalEvmReceiptRlp(badTypedReceipt)) { error in
            XCTAssertEqual(error as? SccpSourceProofHashError, .invalidRlp("receipt.type"))
        }
        var unsupportedTypedReceipt = typedReceipt
        unsupportedTypedReceipt["type"] = "0x7f"
        XCTAssertThrowsError(try canonicalEvmReceiptRlp(unsupportedTypedReceipt)) { error in
            XCTAssertEqual(error as? SccpSourceProofHashError, .invalidRlp("receipt.type"))
        }
        let validReceiptLog: [String: Any] = [
            "address": "0x" + String(repeating: "11", count: 20),
            "topics": ["0x" + String(repeating: "22", count: 32)],
            "data": "0x",
        ]
        var removedLogReceipt = typedReceipt
        removedLogReceipt["logs"] = [validReceiptLog.merging(["removed": true]) { _, new in new }]
        XCTAssertThrowsError(try canonicalEvmReceiptRlp(removedLogReceipt)) { error in
            XCTAssertEqual(error as? SccpSourceProofHashError, .invalidRlp("receipt.logs[0]"))
        }
        var tooManyTopicsLog = validReceiptLog
        tooManyTopicsLog["topics"] = Array(repeating: "0x" + String(repeating: "22", count: 32), count: 5)
        var tooManyTopicsReceipt = typedReceipt
        tooManyTopicsReceipt["logs"] = [tooManyTopicsLog]
        XCTAssertThrowsError(try canonicalEvmReceiptRlp(tooManyTopicsReceipt)) { error in
            XCTAssertEqual(error as? SccpSourceProofHashError, .invalidRlp("receipt.logs[0].topics"))
        }
    }

    func testEthereumMainnetInboundEvidenceUsesMainnetRpcAndRejectsDrift() async throws {
        func assertEvmError(
            _ expected: EvmSccpProverError,
            _ operation: () async throws -> Void
        ) async {
            do {
                try await operation()
                XCTFail("expected \(expected)")
            } catch {
                XCTAssertEqual(error as? EvmSccpProverError, expected)
            }
        }

        let txHash = "0x" + String(repeating: "aa", count: 32)
        let blockHash = "0x" + String(repeating: "bb", count: 32)
        let receipt: [String: Any] = [
            "transactionHash": txHash,
            "blockHash": blockHash,
            "blockNumber": "0x1234",
            "status": "0x1",
        ]
        let sourceEventDigest = "0x" + String(repeating: "ee", count: 32)
        let sourceBridgeAddress = "0x" + String(repeating: "44", count: 20)
        func sourceEventLog(_ overrides: [String: Any] = [:]) -> [String: Any] {
            var log: [String: Any] = [
                "address": sourceBridgeAddress,
                "transactionHash": txHash,
                "blockHash": blockHash,
                "blockNumber": "0x1234",
                "topics": [evmSccpSourceEventTopic(), sourceEventDigest],
                "data": "0x",
            ]
            for (key, value) in overrides {
                log[key] = value
            }
            return log
        }
        var sourceReceipt = receipt
        sourceReceipt["logs"] = [
            [
                "address": "0x" + String(repeating: "11", count: 20),
                "topics": ["0x" + String(repeating: "22", count: 32)],
                "data": "0x1234",
            ],
            sourceEventLog(),
        ]
        var rlpSourceReceipt = sourceReceipt
        rlpSourceReceipt["transactionIndex"] = "0x0"
        rlpSourceReceipt["cumulativeGasUsed"] = "0x5208"
        rlpSourceReceipt["logsBloom"] = "0x" + String(repeating: "00", count: 256)
        let otherReceipt: [String: Any] = [
            "transactionHash": "0x" + String(repeating: "12", count: 32),
            "blockHash": blockHash,
            "blockNumber": "0x1234",
            "transactionIndex": "0x1",
            "status": "0x1",
            "cumulativeGasUsed": "0x5300",
            "logsBloom": "0x" + String(repeating: "00", count: 256),
            "logs": [[String: Any]](),
        ]
        let blockReceipts = [rlpSourceReceipt, otherReceipt]
        let trieProof = try buildEvmReceiptTrieProofFromReceipts(blockReceipts, transactionIndex: "0x0")
        let block: [String: Any] = [
            "hash": blockHash,
            "number": "0x1234",
            "receiptsRoot": "0x" + String(repeating: "cc", count: 32),
        ]
        let beaconFinalityUpdateFields: [String: Any] = [
            "finalityBranch": Self.ethereumFinalityBranch,
            "syncCommitteeBits": Self.ethereumSyncCommitteeSupermajorityBits,
            "syncCommitteeSignature": "0x" + String(repeating: "34", count: 96),
            "syncCommitteeParticipation": Self.ethereumSyncCommitteeSupermajorityParticipation,
            "syncSignatureSlot": "65",
        ]
        let beaconFinalityEvidence = EthereumMainnetBeaconFinalityEvidence(
            executionBlockNumber: "0x1234",
            executionBlockHash: blockHash,
            executionReceiptsRoot: "0x" + String(repeating: "cc", count: 32),
            beaconSlot: "0x20",
            syncCommitteeBits: Self.ethereumSyncCommitteeSupermajorityBits,
            syncCommitteeSignature: "0x" + String(repeating: "34", count: 96),
            syncCommitteeParticipation: Self.ethereumSyncCommitteeSupermajorityParticipation,
            syncSignatureSlot: "65",
            additionalFields: [
                "finalizedHeaderRoot": "0x" + String(repeating: "dd", count: 32),
                "syncCommitteeRoot": "0x" + String(repeating: "aa", count: 32),
                "finalityBranch": Self.ethereumFinalityBranch,
            ]
        )
        let beaconFinality = beaconFinalityEvidence.dictionary
        XCTAssertEqual(beaconFinality["finalityBranch"] as? [String], Self.ethereumFinalityBranch)
        let autoReceiptBlock: [String: Any] = [
            "hash": blockHash,
            "number": "0x1234",
            "receiptsRoot": trieProof.receiptsRoot,
        ]
        let autoReceiptFinality: [String: Any] = [
            "executionBlockNumber": "0x1234",
            "executionBlockHash": blockHash,
            "executionReceiptsRoot": trieProof.receiptsRoot,
            "finalizedHeaderRoot": "0x" + String(repeating: "dd", count: 32),
            "syncCommitteeRoot": "0x" + String(repeating: "aa", count: 32),
            "beaconSlot": "0x20",
            "finalityBranch": Self.ethereumFinalityBranch,
        ].merging(beaconFinalityUpdateFields) { _, new in new }
        let autoReceiptInclusionBranch = [Data(repeating: 0x44, count: 32)]
        let autoReceiptProvider = EthereumMainnetExecutionProviderStub(
            receipt: rlpSourceReceipt,
            block: autoReceiptBlock,
            blockReceipts: blockReceipts
        )
        let autoReceiptEvidence = try await EthereumMainnetSccp(
            executionProvider: autoReceiptProvider
        ).collectInboundEvidenceFromReceipt(
            EthereumMainnetInboundEvidence(
                receipt: rlpSourceReceipt,
                block: autoReceiptBlock,
                beaconFinality: autoReceiptFinality,
                inclusionBranch: autoReceiptInclusionBranch,
                sourceBridgeEmitterAddress: sourceBridgeAddress
            )
        )
        XCTAssertEqual(autoReceiptProvider.calls, ["eth_chainId", "eth_getBlockReceipts"])
        XCTAssertEqual(autoReceiptEvidence.sourceEventDigest, sourceEventDigest)
        XCTAssertEqual(autoReceiptEvidence.blockReceipts?.count, 2)
        let autoReceiptProof = try XCTUnwrap(autoReceiptEvidence.receiptProof)
        XCTAssertEqual(autoReceiptProof.sourceDomain, sccpDomainEthereum)
        XCTAssertEqual(autoReceiptProof.receiptRootIndex, 0)
        XCTAssertEqual(autoReceiptProof.beaconSlot, 32)
        XCTAssertEqual(autoReceiptProof.executionBlockNumber, 0x1234)
        XCTAssertEqual(autoReceiptProof.executionReceiptsRoot, trieProof.receiptsRoot)
        XCTAssertEqual(autoReceiptProof.receiptTrieProofNodes, trieProof.receiptTrieProofNodes)
        XCTAssertEqual(autoReceiptProof.inclusionBranch, autoReceiptInclusionBranch)
        XCTAssertEqual(
            autoReceiptEvidence.receiptProofHash,
            try evmSccpReceiptProofHash(
                sourceEventDigest: autoReceiptProof.sourceEventDigest,
                beaconSlot: autoReceiptProof.beaconSlot,
                executionBlockNumber: autoReceiptProof.executionBlockNumber,
                executionBlockHash: autoReceiptProof.executionBlockHash,
                executionReceiptsRoot: autoReceiptProof.executionReceiptsRoot,
                beaconFinalizedRoot: autoReceiptProof.beaconFinalizedRoot,
                syncCommitteeRoot: autoReceiptProof.syncCommitteeRoot,
                receiptRootIndex: autoReceiptProof.receiptRootIndex,
                receiptTrieProofNodes: autoReceiptProof.receiptTrieProofNodes,
                inclusionBranch: autoReceiptProof.inclusionBranch
            )
        )
        for (missingField, label) in [
            ("finalizedHeaderRoot", "beaconFinality.finalizedHeaderRoot"),
            ("syncCommitteeRoot", "beaconFinality.syncCommitteeRoot"),
            ("beaconSlot", "beaconFinality.beaconSlot"),
        ] {
            var incompleteFinality = autoReceiptFinality
            incompleteFinality.removeValue(forKey: missingField)
            await assertEvmError(.invalidPublicInputs(label)) {
                _ = try await EthereumMainnetSccp().collectInboundEvidenceFromReceipt(
                    EthereumMainnetInboundEvidence(
                        receipt: rlpSourceReceipt,
                        block: autoReceiptBlock,
                        beaconFinality: incompleteFinality,
                        blockReceipts: blockReceipts,
                        inclusionBranch: autoReceiptInclusionBranch,
                        sourceBridgeEmitterAddress: sourceBridgeAddress
                    )
                )
            }
        }
        for (alias, value, label) in [
            ("transaction_hash", "0x" + String(repeating: "ab", count: 32), "receipt.transactionHash"),
            ("block_hash", "0x" + String(repeating: "ab", count: 32), "receipt.blockHash"),
            ("block_number", "0x1235", "receipt.blockNumber"),
            ("transaction_index", "0x0", "receipt.transactionIndex"),
        ] {
            var conflictingReceipt = rlpSourceReceipt
            conflictingReceipt[alias] = value
            await assertEvmError(.invalidPublicInputs(label)) {
                _ = try await EthereumMainnetSccp().collectInboundEvidenceFromReceipt(
                    EthereumMainnetInboundEvidence(
                        receipt: conflictingReceipt,
                        block: autoReceiptBlock,
                        beaconFinality: autoReceiptFinality,
                        blockReceipts: blockReceipts,
                        inclusionBranch: autoReceiptInclusionBranch,
                        sourceBridgeEmitterAddress: sourceBridgeAddress
                    )
                )
            }
        }
        for (alias, value) in [
            ("blockNumber", "0x1235"),
            ("block_number", "0x1235"),
        ] {
            var conflictingBlock = autoReceiptBlock
            conflictingBlock[alias] = value
            await assertEvmError(.invalidPublicInputs("block.number")) {
                _ = try await EthereumMainnetSccp().collectInboundEvidenceFromReceipt(
                    EthereumMainnetInboundEvidence(
                        receipt: rlpSourceReceipt,
                        block: conflictingBlock,
                        beaconFinality: autoReceiptFinality,
                        blockReceipts: blockReceipts,
                        inclusionBranch: autoReceiptInclusionBranch,
                        sourceBridgeEmitterAddress: sourceBridgeAddress
                    )
                )
            }
        }
        var conflictingBlockReceiptsRoot = autoReceiptBlock
        conflictingBlockReceiptsRoot["receipts_root"] = "0x" + String(repeating: "ab", count: 32)
        await assertEvmError(.invalidPublicInputs("block.receiptsRoot")) {
            _ = try await EthereumMainnetSccp().collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receipt: rlpSourceReceipt,
                    block: conflictingBlockReceiptsRoot,
                    beaconFinality: autoReceiptFinality,
                    blockReceipts: blockReceipts,
                    inclusionBranch: autoReceiptInclusionBranch,
                    sourceBridgeEmitterAddress: sourceBridgeAddress
                )
            )
        }
        for (alias, value, label) in [
            ("block_hash", "0x" + String(repeating: "ab", count: 32), "blockReceipts.blockHash"),
            ("block_number", "0x1235", "blockReceipts.blockNumber"),
        ] {
            var conflictingIndexedReceipt = rlpSourceReceipt
            conflictingIndexedReceipt[alias] = value
            await assertEvmError(.invalidPublicInputs(label)) {
                _ = try await EthereumMainnetSccp().collectInboundEvidenceFromReceipt(
                    EthereumMainnetInboundEvidence(
                        receipt: rlpSourceReceipt,
                        block: autoReceiptBlock,
                        beaconFinality: autoReceiptFinality,
                        blockReceipts: [conflictingIndexedReceipt, otherReceipt],
                        inclusionBranch: autoReceiptInclusionBranch,
                        sourceBridgeEmitterAddress: sourceBridgeAddress
                    )
                )
            }
        }
        var conflictingIndexedHashReceipt = rlpSourceReceipt
        conflictingIndexedHashReceipt["transaction_hash"] = rlpSourceReceipt["transactionHash"]
        do {
            _ = try await EthereumMainnetSccp().collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receipt: rlpSourceReceipt,
                    block: autoReceiptBlock,
                    beaconFinality: autoReceiptFinality,
                    blockReceipts: [conflictingIndexedHashReceipt, otherReceipt],
                    inclusionBranch: autoReceiptInclusionBranch,
                    sourceBridgeEmitterAddress: sourceBridgeAddress
                )
            )
            XCTFail("expected blockReceipts[0].transactionHash")
        } catch {
            XCTAssertEqual(
                error as? SccpSourceProofHashError,
                .invalidRlp("blockReceipts[0].transactionHash")
            )
        }
        await assertEvmError(.invalidPublicInputs("receiptProof.executionReceiptsRoot")) {
            _ = try await EthereumMainnetSccp().collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receipt: rlpSourceReceipt,
                    block: [
                        "hash": blockHash,
                        "number": "0x1234",
                        "receiptsRoot": "0x" + String(repeating: "99", count: 32),
                    ],
                    beaconFinality: autoReceiptFinality.merging([
                        "executionReceiptsRoot": "0x" + String(repeating: "99", count: 32),
                    ]) { _, new in new },
                    blockReceipts: blockReceipts,
                    inclusionBranch: autoReceiptInclusionBranch,
                    sourceBridgeEmitterAddress: sourceBridgeAddress
                )
            )
        }
        var mismatchedIndexedReceipt = rlpSourceReceipt
        mismatchedIndexedReceipt["logs"] = [[String: Any]]()
        let mismatchedBlockReceipts = [mismatchedIndexedReceipt, otherReceipt]
        let mismatchedReceiptProof = try buildEvmReceiptTrieProofFromReceipts(
            mismatchedBlockReceipts,
            transactionIndex: "0x0"
        )
        await assertEvmError(.invalidPublicInputs("blockReceipts.receiptRlp")) {
            _ = try await EthereumMainnetSccp().collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receipt: rlpSourceReceipt,
                    block: [
                        "hash": blockHash,
                        "number": "0x1234",
                        "receiptsRoot": mismatchedReceiptProof.receiptsRoot,
                    ],
                    beaconFinality: autoReceiptFinality.merging([
                        "executionReceiptsRoot": mismatchedReceiptProof.receiptsRoot,
                    ]) { _, new in new },
                    blockReceipts: mismatchedBlockReceipts,
                    inclusionBranch: autoReceiptInclusionBranch,
                    sourceBridgeEmitterAddress: sourceBridgeAddress
                )
            )
        }
        var blockHashDriftReceipt = rlpSourceReceipt
        blockHashDriftReceipt["blockHash"] = "0x" + String(repeating: "99", count: 32)
        await assertEvmError(.invalidPublicInputs("blockReceipts.blockHash")) {
            _ = try await EthereumMainnetSccp().collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receipt: rlpSourceReceipt,
                    block: autoReceiptBlock,
                    beaconFinality: autoReceiptFinality,
                    blockReceipts: [blockHashDriftReceipt, otherReceipt],
                    inclusionBranch: autoReceiptInclusionBranch,
                    sourceBridgeEmitterAddress: sourceBridgeAddress
                )
            )
        }
        var blockNumberDriftReceipt = rlpSourceReceipt
        blockNumberDriftReceipt["blockNumber"] = "0x1235"
        await assertEvmError(.invalidPublicInputs("blockReceipts.blockNumber")) {
            _ = try await EthereumMainnetSccp().collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receipt: rlpSourceReceipt,
                    block: autoReceiptBlock,
                    beaconFinality: autoReceiptFinality,
                    blockReceipts: [blockNumberDriftReceipt, otherReceipt],
                    inclusionBranch: autoReceiptInclusionBranch,
                    sourceBridgeEmitterAddress: sourceBridgeAddress
                )
            )
        }
        let receiptProof = EthereumMainnetReceiptProof(
            sourceEventDigest: sourceEventDigest,
            beaconSlot: 32,
            executionBlockNumber: 0x1234,
            executionBlockHash: blockHash,
            executionReceiptsRoot: "0x" + String(repeating: "cc", count: 32),
            beaconFinalizedRoot: "0x" + String(repeating: "dd", count: 32),
            syncCommitteeRoot: "0x" + String(repeating: "aa", count: 32),
            receiptRootIndex: 3,
            receiptTrieProofNodes: [Data([0x01]), Data([0x02, 0x03])],
            inclusionBranch: [Data(repeating: 0x11, count: 32)]
        )
        let receiptProofHash = try evmSccpReceiptProofHash(
            sourceEventDigest: receiptProof.sourceEventDigest,
            beaconSlot: receiptProof.beaconSlot,
            executionBlockNumber: receiptProof.executionBlockNumber,
            executionBlockHash: receiptProof.executionBlockHash,
            executionReceiptsRoot: receiptProof.executionReceiptsRoot,
            beaconFinalizedRoot: receiptProof.beaconFinalizedRoot,
            syncCommitteeRoot: receiptProof.syncCommitteeRoot,
            receiptRootIndex: receiptProof.receiptRootIndex,
            receiptTrieProofNodes: receiptProof.receiptTrieProofNodes,
            inclusionBranch: receiptProof.inclusionBranch
        )
        XCTAssertEqual(
            receiptProofHash,
            "0x39f014e3f5f8d38b44d59f1afdf72ceb71d10d6d937f268c404b046f092b38f0"
        )
        let zeroHash = "0x" + String(repeating: "00", count: 32)
        XCTAssertThrowsError(try canonicalEvmReceiptRootMptValue(receiptRoot: zeroHash)) { error in
            XCTAssertEqual(error as? SccpSourceProofHashError, .invalidHex32("receiptRoot"))
        }
        XCTAssertThrowsError(try evmSccpReceiptProofHash(
            sourceEventDigest: receiptProof.sourceEventDigest,
            beaconSlot: receiptProof.beaconSlot,
            executionBlockNumber: receiptProof.executionBlockNumber,
            executionBlockHash: receiptProof.executionBlockHash,
            executionReceiptsRoot: receiptProof.executionReceiptsRoot,
            beaconFinalizedRoot: receiptProof.beaconFinalizedRoot,
            syncCommitteeRoot: receiptProof.syncCommitteeRoot,
            receiptRootIndex: receiptProof.receiptRootIndex,
            receiptTrieProofNodes: [],
            inclusionBranch: receiptProof.inclusionBranch
        )) { error in
            XCTAssertEqual(error as? SccpSourceProofHashError, .invalidValidatorSet("receiptTrieProofNodes"))
        }
        XCTAssertThrowsError(try evmSccpReceiptProofHash(
            sourceEventDigest: receiptProof.sourceEventDigest,
            beaconSlot: receiptProof.beaconSlot,
            executionBlockNumber: receiptProof.executionBlockNumber,
            executionBlockHash: receiptProof.executionBlockHash,
            executionReceiptsRoot: receiptProof.executionReceiptsRoot,
            beaconFinalizedRoot: receiptProof.beaconFinalizedRoot,
            syncCommitteeRoot: receiptProof.syncCommitteeRoot,
            receiptRootIndex: receiptProof.receiptRootIndex,
            receiptTrieProofNodes: receiptProof.receiptTrieProofNodes,
            inclusionBranch: []
        )) { error in
            XCTAssertEqual(error as? SccpSourceProofHashError, .invalidBranch("inclusionBranch"))
        }
        let provider = EthereumMainnetExecutionProviderStub(receipt: receipt, block: block)
        let consensusProvider = EthereumMainnetConsensusProviderStub(finality: beaconFinality)
        let sdk = EthereumMainnetSccp(
            executionProvider: provider,
            consensusProvider: consensusProvider,
            inboundProveFunction: { evidence in
                XCTAssertEqual(evidence.sourceDomain, sccpDomainEthereum)
                XCTAssertEqual(evidence.targetDomain, sccpDomainSora)
                XCTAssertEqual(evidence.transactionHash, txHash)
                XCTAssertEqual(evidence.beaconFinality?["finalizedHeaderRoot"] as? String, beaconFinality["finalizedHeaderRoot"] as? String)
                XCTAssertEqual(evidence.beaconFinality?["syncCommitteeRoot"] as? String, beaconFinality["syncCommitteeRoot"] as? String)
                XCTAssertEqual(evidence.beaconFinality?["beaconSlot"] as? String, "32")
                XCTAssertEqual(evidence.beaconFinality?["executionBlockNumber"] as? String, "4660")
                XCTAssertEqual(evidence.beaconFinality?["executionBlockHash"] as? String, blockHash)
                XCTAssertEqual(evidence.receiptProofHash, receiptProofHash)
                XCTAssertEqual(evidence.receiptProof?.sourceEventDigest, receiptProof.sourceEventDigest)
                XCTAssertEqual(evidence.sourceEventDigest, sourceEventDigest)
                return Data([1, 2, 3])
            },
            inboundSubmitFunction: { proofBytes in
                XCTAssertEqual(proofBytes, Data([1, 2, 3]))
                return "submitted"
            }
        )

        let evidence = try await sdk.collectInboundEvidenceFromReceipt(
            EthereumMainnetInboundEvidence(transactionHash: txHash)
        )
        XCTAssertEqual(evidence.transactionHash, txHash)
        XCTAssertEqual(evidence.receipt?["status"] as? String, "0x1")
        XCTAssertEqual(evidence.block?["receiptsRoot"] as? String, "0x" + String(repeating: "cc", count: 32))
        XCTAssertEqual(evidence.beaconFinality?["finalizedHeaderRoot"] as? String, beaconFinality["finalizedHeaderRoot"] as? String)
        XCTAssertEqual(evidence.beaconFinality?["syncCommitteeRoot"] as? String, beaconFinality["syncCommitteeRoot"] as? String)
        XCTAssertEqual(evidence.beaconFinality?["beaconSlot"] as? String, "32")
        XCTAssertEqual(evidence.beaconFinality?["finalityBranch"] as? [String], Self.ethereumFinalityBranch)
        XCTAssertEqual(evidence.beaconFinality?["executionReceiptsRoot"] as? String, "0x" + String(repeating: "cc", count: 32))
        XCTAssertEqual(consensusProvider.calls, 1)
        XCTAssertEqual(provider.calls, ["eth_chainId", "eth_getTransactionReceipt", "eth_getBlockByHash"])
        await assertEvmError(.invalidPublicInputs("receipt.sourceEvent")) {
            _ = try await sdk.proveInboundToSora(
                EthereumMainnetInboundEvidence(
                    transactionHash: evidence.transactionHash,
                    receipt: evidence.receipt,
                    block: evidence.block,
                    beaconFinality: evidence.beaconFinality,
                    receiptProof: receiptProof,
                    receiptProofHash: receiptProofHash
                )
            )
        }
        let proofReadyEvidence = EthereumMainnetInboundEvidence(
            transactionHash: evidence.transactionHash,
            receipt: sourceReceipt,
            block: evidence.block,
            beaconFinality: evidence.beaconFinality,
            receiptProof: receiptProof,
            receiptProofHash: receiptProofHash,
            sourceBridgeEmitterAddress: sourceBridgeAddress
        )
        XCTAssertEqual(proofReadyEvidence.beaconFinality?["finalityBranch"] as? [String], Self.ethereumFinalityBranch)
        let recollectedProofReadyEvidence = try await EthereumMainnetSccp().collectInboundEvidenceFromReceipt(proofReadyEvidence)
        XCTAssertEqual(recollectedProofReadyEvidence.beaconFinality?["finalityBranch"] as? [String], Self.ethereumFinalityBranch)
        let proofBytes = try await sdk.proveInboundToSora(proofReadyEvidence)
        XCTAssertEqual(proofBytes, Data([1, 2, 3]))
        var missingFinalityBranchFinality = try XCTUnwrap(evidence.beaconFinality)
        missingFinalityBranchFinality.removeValue(forKey: "finalityBranch")
        await assertEvmError(.invalidPublicInputs("beaconFinality.finalityBranch")) {
            _ = try await sdk.proveInboundToSora(
                EthereumMainnetInboundEvidence(
                    transactionHash: evidence.transactionHash,
                    receipt: sourceReceipt,
                    block: evidence.block,
                    beaconFinality: missingFinalityBranchFinality,
                    receiptProof: receiptProof,
                    receiptProofHash: receiptProofHash,
                    sourceBridgeEmitterAddress: sourceBridgeAddress
                )
            )
        }
        var missingSyncBitsFinality = try XCTUnwrap(evidence.beaconFinality)
        missingSyncBitsFinality.removeValue(forKey: "syncCommitteeBits")
        await assertEvmError(.invalidPublicInputs("beaconFinality.syncCommitteeBits")) {
            _ = try await sdk.proveInboundToSora(
                EthereumMainnetInboundEvidence(
                    transactionHash: evidence.transactionHash,
                    receipt: sourceReceipt,
                    block: evidence.block,
                    beaconFinality: missingSyncBitsFinality,
                    receiptProof: receiptProof,
                    receiptProofHash: receiptProofHash,
                    sourceBridgeEmitterAddress: sourceBridgeAddress
                )
            )
        }
        var conflictingSyncBitsFinality = try XCTUnwrap(evidence.beaconFinality)
        conflictingSyncBitsFinality["sync_committee_bits"] = "0x02" + String(repeating: "00", count: 63)
        await assertEvmError(.invalidPublicInputs("beaconFinality.syncCommitteeBits")) {
            _ = try await sdk.proveInboundToSora(
                EthereumMainnetInboundEvidence(
                    transactionHash: evidence.transactionHash,
                    receipt: sourceReceipt,
                    block: evidence.block,
                    beaconFinality: conflictingSyncBitsFinality,
                    receiptProof: receiptProof,
                    receiptProofHash: receiptProofHash,
                    sourceBridgeEmitterAddress: sourceBridgeAddress
                )
            )
        }
        var mismatchedSyncParticipationFinality = try XCTUnwrap(evidence.beaconFinality)
        mismatchedSyncParticipationFinality["syncCommitteeParticipation"] = "341"
        await assertEvmError(.invalidPublicInputs("beaconFinality.syncCommitteeParticipation")) {
            _ = try await sdk.proveInboundToSora(
                EthereumMainnetInboundEvidence(
                    transactionHash: evidence.transactionHash,
                    receipt: sourceReceipt,
                    block: evidence.block,
                    beaconFinality: mismatchedSyncParticipationFinality,
                    receiptProof: receiptProof,
                    receiptProofHash: receiptProofHash,
                    sourceBridgeEmitterAddress: sourceBridgeAddress
                )
            )
        }
        var underQuorumSyncBitsFinality = try XCTUnwrap(evidence.beaconFinality)
        underQuorumSyncBitsFinality["syncCommitteeBits"] = "0x01" + String(repeating: "00", count: 63)
        underQuorumSyncBitsFinality["syncCommitteeParticipation"] = "1"
        await assertEvmError(.invalidPublicInputs("beaconFinality.syncCommitteeBits")) {
            _ = try await sdk.proveInboundToSora(
                EthereumMainnetInboundEvidence(
                    transactionHash: evidence.transactionHash,
                    receipt: sourceReceipt,
                    block: evidence.block,
                    beaconFinality: underQuorumSyncBitsFinality,
                    receiptProof: receiptProof,
                    receiptProofHash: receiptProofHash,
                    sourceBridgeEmitterAddress: sourceBridgeAddress
                )
            )
        }
        var staleSyncSignatureSlotFinality = try XCTUnwrap(evidence.beaconFinality)
        staleSyncSignatureSlotFinality["syncSignatureSlot"] = "31"
        await assertEvmError(.invalidPublicInputs("beaconFinality.syncSignatureSlot")) {
            _ = try await sdk.proveInboundToSora(
                EthereumMainnetInboundEvidence(
                    transactionHash: evidence.transactionHash,
                    receipt: sourceReceipt,
                    block: evidence.block,
                    beaconFinality: staleSyncSignatureSlotFinality,
                    receiptProof: receiptProof,
                    receiptProofHash: receiptProofHash,
                    sourceBridgeEmitterAddress: sourceBridgeAddress
                )
            )
        }
        var zeroSyncCommitteeSignatureFinality = try XCTUnwrap(evidence.beaconFinality)
        zeroSyncCommitteeSignatureFinality["syncCommitteeSignature"] = "0x" + String(repeating: "00", count: 96)
        await assertEvmError(.zeroField("beaconFinality.syncCommitteeSignature")) {
            _ = try await sdk.proveInboundToSora(
                EthereumMainnetInboundEvidence(
                    transactionHash: evidence.transactionHash,
                    receipt: sourceReceipt,
                    block: evidence.block,
                    beaconFinality: zeroSyncCommitteeSignatureFinality,
                    receiptProof: receiptProof,
                    receiptProofHash: receiptProofHash,
                    sourceBridgeEmitterAddress: sourceBridgeAddress
                )
            )
        }
        let aliasOnlyFinality: [String: Any] = [
            "execution_block_number": "0x1234",
            "finality_block_hash": blockHash,
            "receipts_root": "0x" + String(repeating: "cc", count: 32),
            "finalized_header_root": "0x" + String(repeating: "dd", count: 32),
            "sync_committee_root": "0x" + String(repeating: "aa", count: 32),
            "beacon_slot": "0x20",
            "finality_branch": Self.ethereumFinalityBranch,
            "sync_committee_bits": Self.ethereumSyncCommitteeSupermajorityBits,
            "sync_committee_signature": "0x" + String(repeating: "34", count: 96),
            "sync_committee_participation": Self.ethereumSyncCommitteeSupermajorityParticipation,
            "signature_slot": "65",
            "extensionWitness": "kept",
        ]
        let aliasOnlyProof = try await EthereumMainnetSccp(
            inboundProveFunction: { evidence in
                let finality = try XCTUnwrap(evidence.beaconFinality)
                XCTAssertEqual(finality["executionBlockNumber"] as? String, "4660")
                XCTAssertEqual(finality["executionBlockHash"] as? String, blockHash)
                XCTAssertEqual(finality["executionReceiptsRoot"] as? String, "0x" + String(repeating: "cc", count: 32))
                XCTAssertEqual(finality["finalizedHeaderRoot"] as? String, "0x" + String(repeating: "dd", count: 32))
                XCTAssertEqual(finality["syncCommitteeRoot"] as? String, "0x" + String(repeating: "aa", count: 32))
                XCTAssertEqual(finality["beaconSlot"] as? String, "32")
                XCTAssertEqual(finality["finalityBranch"] as? [String], Self.ethereumFinalityBranch)
                XCTAssertEqual(finality["syncCommitteeBits"] as? String, Self.ethereumSyncCommitteeSupermajorityBits)
                XCTAssertEqual(finality["syncCommitteeSignature"] as? String, "0x" + String(repeating: "34", count: 96))
                XCTAssertEqual(finality["syncCommitteeParticipation"] as? String, Self.ethereumSyncCommitteeSupermajorityParticipation)
                XCTAssertEqual(finality["syncSignatureSlot"] as? String, "65")
                XCTAssertEqual(finality["extensionWitness"] as? String, "kept")
                for alias in [
                    "execution_block_number",
                    "finalityHeight",
                    "finality_block_hash",
                    "receipts_root",
                    "finalized_header_root",
                    "sync_committee_root",
                    "beacon_slot",
                    "finality_branch",
                    "sync_committee_bits",
                    "sync_committee_signature",
                    "sync_committee_participation",
                    "signature_slot",
                ] {
                    XCTAssertFalse(finality.keys.contains(alias))
                }
                return Data([4, 5, 6])
            }
        ).proveInboundToSora(
            EthereumMainnetInboundEvidence(
                transactionHash: evidence.transactionHash,
                receipt: sourceReceipt,
                block: evidence.block,
                beaconFinality: aliasOnlyFinality,
                receiptProof: receiptProof,
                receiptProofHash: receiptProofHash,
                sourceBridgeEmitterAddress: sourceBridgeAddress
            )
        )
        XCTAssertEqual(aliasOnlyProof, Data([4, 5, 6]))
        for (alias, value, label) in [
            ("finalized_header_root", "0x" + String(repeating: "13", count: 32), "beaconFinality.finalizedHeaderRoot"),
            ("sync_committee_root", "0x" + String(repeating: "14", count: 32), "beaconFinality.syncCommitteeRoot"),
            ("beacon_slot", "33", "beaconFinality.beaconSlot"),
        ] {
            var conflictingFinality = try XCTUnwrap(evidence.beaconFinality)
            conflictingFinality[alias] = value
            await assertEvmError(.invalidPublicInputs(label)) {
                _ = try await sdk.proveInboundToSora(
                    EthereumMainnetInboundEvidence(
                        transactionHash: evidence.transactionHash,
                        receipt: sourceReceipt,
                        block: evidence.block,
                        beaconFinality: conflictingFinality,
                        receiptProof: receiptProof,
                        receiptProofHash: receiptProofHash,
                        sourceBridgeEmitterAddress: sourceBridgeAddress
                    )
                )
            }
        }
        await assertEvmError(.emptyProof) {
            _ = try await EthereumMainnetSccp(inboundProveFunction: { _ in Data() })
                .proveInboundToSora(proofReadyEvidence)
        }
        await assertEvmError(.allZeroProof) {
            _ = try await EthereumMainnetSccp(inboundProveFunction: { _ in Data([0, 0]) })
                .proveInboundToSora(proofReadyEvidence)
        }
        let oversizedInboundProof = Data(repeating: 1, count: sccpNativeRecursiveMaxProofBytes + 1)
        await assertEvmError(.invalidPublicInputs("proofBytes")) {
            _ = try await EthereumMainnetSccp(inboundProveFunction: { _ in oversizedInboundProof })
                .proveInboundToSora(proofReadyEvidence)
        }
        await assertEvmError(.invalidPublicInputs("proofBytes")) {
            _ = try await sdk.submitInboundToIroha(oversizedInboundProof)
        }
        let submitResult = try await sdk.submitInboundToIroha(Data([1, 2, 3]))
        XCTAssertEqual(submitResult as? String, "submitted")

        let typedFinalityProof = try await EthereumMainnetSccp(
            inboundProveFunction: { evidence in
                XCTAssertEqual(evidence.transactionHash, txHash)
                XCTAssertEqual(evidence.beaconFinality?["executionBlockHash"] as? String, blockHash)
                return Data([7, 8, 9])
            }
        ).proveInboundToSora(
            EthereumMainnetInboundEvidence(
                receipt: sourceReceipt,
                block: block,
                beaconFinalityEvidence: beaconFinalityEvidence,
                receiptProof: receiptProof,
                receiptProofHash: receiptProofHash,
                sourceBridgeEmitterAddress: sourceBridgeAddress
            )
        )
        XCTAssertEqual(typedFinalityProof, Data([7, 8, 9]))

        let receiptProofEvidence = try await EthereumMainnetSccp().collectInboundEvidenceFromReceipt(
            EthereumMainnetInboundEvidence(
                receiptProof: receiptProof,
                receiptProofHash: receiptProofHash
            )
        )
        XCTAssertEqual(receiptProofEvidence.receiptProofHash, receiptProofHash)
        XCTAssertEqual(receiptProofEvidence.receiptProof?.sourceEventDigest, receiptProof.sourceEventDigest)
        let receiptProofHashOnlyEvidence = try await EthereumMainnetSccp().collectInboundEvidenceFromReceipt(
            EthereumMainnetInboundEvidence(receiptProofHash: receiptProofHash)
        )
        XCTAssertEqual(receiptProofHashOnlyEvidence.receiptProofHash, receiptProofHash)
        XCTAssertNil(receiptProofHashOnlyEvidence.receiptProof)
        await assertEvmError(.zeroField("receiptProofHash")) {
            _ = try await EthereumMainnetSccp().collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receiptProofHash: "0x" + String(repeating: "00", count: 32)
                )
            )
        }
        await assertEvmError(.invalidPublicInputs("receiptProofHash")) {
            _ = try await EthereumMainnetSccp().collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(receiptProofHash: receiptProofHash + " ")
            )
        }
        await assertEvmError(.invalidPublicInputs("receipt.logs")) {
            _ = try await EthereumMainnetSccp().collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receiptProof: receiptProof,
                    sourceEventDigest: sourceEventDigest
                )
            )
        }
        await assertEvmError(.invalidPublicInputs("receiptProofHash")) {
            _ = try await EthereumMainnetSccp().collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receiptProof: receiptProof,
                    receiptProofHash: "0x" + String(repeating: "99", count: 32)
                )
            )
        }
        await assertEvmError(.invalidPublicInputs("receiptProof.sourceDomain")) {
            _ = try await EthereumMainnetSccp().collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receiptProof: EthereumMainnetReceiptProof(
                        sourceDomain: sccpDomainBsc,
                        sourceEventDigest: receiptProof.sourceEventDigest,
                        beaconSlot: receiptProof.beaconSlot,
                        executionBlockNumber: receiptProof.executionBlockNumber,
                        executionBlockHash: receiptProof.executionBlockHash,
                        executionReceiptsRoot: receiptProof.executionReceiptsRoot,
                        beaconFinalizedRoot: receiptProof.beaconFinalizedRoot,
                        syncCommitteeRoot: receiptProof.syncCommitteeRoot,
                        receiptRootIndex: receiptProof.receiptRootIndex,
                        receiptTrieProofNodes: receiptProof.receiptTrieProofNodes,
                        inclusionBranch: receiptProof.inclusionBranch
                    )
                )
            )
        }
        let sourceEvidence = try await EthereumMainnetSccp().collectInboundEvidenceFromReceipt(
            EthereumMainnetInboundEvidence(
                receipt: sourceReceipt,
                block: block,
                sourceBridgeEmitterAddress: sourceBridgeAddress
            )
        )
        XCTAssertEqual(sourceEvidence.sourceEventDigest, sourceEventDigest)
        XCTAssertEqual(sourceEvidence.sourceBridgeEmitterAddress, sourceBridgeAddress)
        let explicitSourceEvidence = try await EthereumMainnetSccp().collectInboundEvidenceFromReceipt(
            EthereumMainnetInboundEvidence(
                receipt: sourceReceipt,
                block: block,
                sourceEventDigest: sourceEventDigest,
                sourceBridgeEmitterAddress: sourceBridgeAddress
            )
        )
        XCTAssertEqual(explicitSourceEvidence.sourceEventDigest, sourceEventDigest)
        let configuredSourceEvidence = try await EthereumMainnetSccp(
            sourceBridgeEmitterAddress: sourceBridgeAddress
        ).collectInboundEvidenceFromReceipt(
            EthereumMainnetInboundEvidence(
                receipt: sourceReceipt,
                block: block
            )
        )
        XCTAssertEqual(configuredSourceEvidence.sourceEventDigest, sourceEventDigest)
        XCTAssertEqual(configuredSourceEvidence.sourceBridgeEmitterAddress, sourceBridgeAddress)
        await assertEvmError(.invalidPublicInputs("sourceBridgeEmitterAddress")) {
            _ = try await EthereumMainnetSccp(
                sourceBridgeEmitterAddress: sourceBridgeAddress
            ).collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receipt: sourceReceipt,
                    block: block,
                    sourceBridgeEmitterAddress: "0x" + String(repeating: "45", count: 20)
                )
            )
        }

        await assertEvmError(.invalidPublicInputs("sourceBridgeEmitterAddress")) {
            _ = try await EthereumMainnetSccp().collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receipt: sourceReceipt,
                    block: block,
                    sourceEventDigest: sourceEventDigest
                )
            )
        }
        await assertEvmError(.invalidPublicInputs("receipt.logs")) {
            _ = try await EthereumMainnetSccp().collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receipt: sourceReceipt,
                    block: block,
                    sourceBridgeEmitterAddress: "0x" + String(repeating: "45", count: 20)
                )
            )
        }
        var wrongTopicReceipt = sourceReceipt
        wrongTopicReceipt["logs"] = [
            sourceEventLog([
                "topics": ["0x" + String(repeating: "99", count: 32), sourceEventDigest],
            ]),
        ]
        await assertEvmError(.invalidPublicInputs("receipt.logs")) {
            _ = try await EthereumMainnetSccp().collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receipt: wrongTopicReceipt,
                    block: block,
                    sourceBridgeEmitterAddress: sourceBridgeAddress
                )
            )
        }
        var extraTopicReceipt = sourceReceipt
        extraTopicReceipt["logs"] = [
            sourceEventLog([
                "topics": [evmSccpSourceEventTopic(), sourceEventDigest, "0x" + String(repeating: "66", count: 32)],
            ]),
        ]
        await assertEvmError(.invalidPublicInputs("receipt.logs[0].topics")) {
            _ = try await EthereumMainnetSccp().collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receipt: extraTopicReceipt,
                    block: block,
                    sourceBridgeEmitterAddress: sourceBridgeAddress
                )
            )
        }
        var nonEmptyDataReceipt = sourceReceipt
        nonEmptyDataReceipt["logs"] = [sourceEventLog(["data": "0x01"])]
        await assertEvmError(.invalidPublicInputs("receipt.logs[0].data")) {
            _ = try await EthereumMainnetSccp().collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receipt: nonEmptyDataReceipt,
                    block: block,
                    sourceBridgeEmitterAddress: sourceBridgeAddress
                )
            )
        }
        var zeroDigestReceipt = sourceReceipt
        zeroDigestReceipt["logs"] = [
            sourceEventLog([
                "topics": [evmSccpSourceEventTopic(), "0x" + String(repeating: "00", count: 32)],
            ]),
        ]
        await assertEvmError(.invalidPublicInputs("receipt.logs[0].topics[1]")) {
            _ = try await EthereumMainnetSccp().collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receipt: zeroDigestReceipt,
                    block: block,
                    sourceBridgeEmitterAddress: sourceBridgeAddress
                )
            )
        }
        var duplicateLogReceipt = sourceReceipt
        duplicateLogReceipt["logs"] = [sourceEventLog(), sourceEventLog()]
        await assertEvmError(.invalidPublicInputs("receipt.logs")) {
            _ = try await EthereumMainnetSccp().collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receipt: duplicateLogReceipt,
                    block: block,
                    sourceBridgeEmitterAddress: sourceBridgeAddress
                )
            )
        }
        var removedLogReceipt = sourceReceipt
        removedLogReceipt["logs"] = [sourceEventLog(["removed": true])]
        await assertEvmError(.invalidPublicInputs("receipt.logs")) {
            _ = try await EthereumMainnetSccp().collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receipt: removedLogReceipt,
                    block: block,
                    sourceBridgeEmitterAddress: sourceBridgeAddress
                )
            )
        }
        var nonObjectLogReceipt = sourceReceipt
        nonObjectLogReceipt["logs"] = ["not-a-log"]
        await assertEvmError(.invalidPublicInputs("receipt.logs")) {
            _ = try await EthereumMainnetSccp().collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receipt: nonObjectLogReceipt,
                    block: block,
                    sourceBridgeEmitterAddress: sourceBridgeAddress
                )
            )
        }
        var missingDataLog = sourceEventLog()
        missingDataLog.removeValue(forKey: "data")
        var missingDataReceipt = sourceReceipt
        missingDataReceipt["logs"] = [missingDataLog]
        await assertEvmError(.invalidPublicInputs("receipt.logs[0].data")) {
            _ = try await EthereumMainnetSccp().collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receipt: missingDataReceipt,
                    block: block,
                    sourceBridgeEmitterAddress: sourceBridgeAddress
                )
            )
        }
        for missingField in ["transactionHash", "blockHash", "blockNumber"] {
            var missingContextLog = sourceEventLog()
            missingContextLog.removeValue(forKey: missingField)
            var missingContextReceipt = sourceReceipt
            missingContextReceipt["logs"] = [missingContextLog]
            await assertEvmError(.invalidPublicInputs("receipt.logs[0].\(missingField)")) {
                _ = try await EthereumMainnetSccp().collectInboundEvidenceFromReceipt(
                    EthereumMainnetInboundEvidence(
                        receipt: missingContextReceipt,
                        block: block,
                        sourceBridgeEmitterAddress: sourceBridgeAddress
                    )
                )
            }
        }
        for (alias, value, label) in [
            ("transaction_hash", "0x" + String(repeating: "ab", count: 32), "receipt.logs[0].transactionHash"),
            ("block_hash", "0x" + String(repeating: "ac", count: 32), "receipt.logs[0].blockHash"),
            ("block_number", "0x1235", "receipt.logs[0].blockNumber"),
        ] {
            var conflictingContextReceipt = sourceReceipt
            conflictingContextReceipt["logs"] = [
                sourceEventLog([alias: value]),
            ]
            await assertEvmError(.invalidPublicInputs(label)) {
                _ = try await EthereumMainnetSccp().collectInboundEvidenceFromReceipt(
                    EthereumMainnetInboundEvidence(
                        receipt: conflictingContextReceipt,
                        block: block,
                        sourceBridgeEmitterAddress: sourceBridgeAddress
                    )
                )
            }
        }
        var driftedLogTransactionReceipt = sourceReceipt
        driftedLogTransactionReceipt["logs"] = [
            sourceEventLog(["transactionHash": "0x" + String(repeating: "ab", count: 32)]),
        ]
        await assertEvmError(.invalidPublicInputs("receipt.logs")) {
            _ = try await EthereumMainnetSccp().collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receipt: driftedLogTransactionReceipt,
                    block: block,
                    sourceBridgeEmitterAddress: sourceBridgeAddress
                )
            )
        }
        var driftedLogBlockHashReceipt = sourceReceipt
        driftedLogBlockHashReceipt["logs"] = [
            sourceEventLog(["blockHash": "0x" + String(repeating: "ab", count: 32)]),
        ]
        await assertEvmError(.invalidPublicInputs("receipt.logs")) {
            _ = try await EthereumMainnetSccp().collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receipt: driftedLogBlockHashReceipt,
                    block: block,
                    sourceBridgeEmitterAddress: sourceBridgeAddress
                )
            )
        }
        var driftedLogBlockNumberReceipt = sourceReceipt
        driftedLogBlockNumberReceipt["logs"] = [
            sourceEventLog(["blockNumber": "0x1235"]),
        ]
        await assertEvmError(.invalidPublicInputs("receipt.logs")) {
            _ = try await EthereumMainnetSccp().collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    receipt: driftedLogBlockNumberReceipt,
                    block: block,
                    sourceBridgeEmitterAddress: sourceBridgeAddress
                )
            )
        }

        let perCallProvider = EthereumMainnetExecutionProviderStub(receipt: sourceReceipt, block: block)
        let perCallConsensusProvider = EthereumMainnetConsensusProviderStub(finality: beaconFinality)
        let perCallSdk = EthereumMainnetSccp(
            inboundProveFunction: { evidence in
                XCTAssertEqual(evidence.transactionHash, txHash)
                XCTAssertEqual(evidence.beaconFinality?["executionBlockHash"] as? String, blockHash)
                return Data([4, 5, 6])
            }
        )
        let perCallProof = try await perCallSdk.proveInboundToSora(
            EthereumMainnetInboundEvidence(
                transactionHash: txHash,
                receiptProof: receiptProof,
                receiptProofHash: receiptProofHash,
                sourceBridgeEmitterAddress: sourceBridgeAddress
            ),
            executionProvider: perCallProvider,
            consensusProvider: perCallConsensusProvider
        )
        XCTAssertEqual(perCallProof, Data([4, 5, 6]))
        XCTAssertEqual(perCallConsensusProvider.calls, 1)
        XCTAssertEqual(perCallProvider.calls, ["eth_chainId", "eth_getTransactionReceipt", "eth_getBlockByHash"])

        await assertEvmError(.invalidPublicInputs("receipt.sourceEvent")) {
            _ = try await EthereumMainnetSccp(
                inboundProveFunction: { _ in
                    XCTFail("prover callback must not run without source event validation")
                    return Data([1, 2, 3])
                }
            ).proveInboundToSora(
                EthereumMainnetInboundEvidence(
                    beaconFinality: beaconFinality,
                    receiptProof: receiptProof,
                    receiptProofHash: receiptProofHash
                )
            )
        }

        await assertEvmError(.invalidPublicInputs("beaconFinality")) {
            _ = try await EthereumMainnetSccp(
                inboundProveFunction: { _ in
                    XCTFail("prover callback must not run without beaconFinality")
                    return Data([1, 2, 3])
                }
            ).proveInboundToSora(EthereumMainnetInboundEvidence(receipt: receipt, block: block))
        }

        await assertEvmError(.invalidPublicInputs("receiptProof")) {
            _ = try await EthereumMainnetSccp(
                inboundProveFunction: { _ in
                    XCTFail("prover callback must not run without receiptProof")
                    return Data([1, 2, 3])
                }
            ).proveInboundToSora(
                EthereumMainnetInboundEvidence(
                    receipt: receipt,
                    block: block,
                    beaconFinality: beaconFinality,
                    receiptProofHash: receiptProofHash
                )
            )
        }

        let driftedReceiptProof = EthereumMainnetReceiptProof(
            sourceEventDigest: receiptProof.sourceEventDigest,
            beaconSlot: receiptProof.beaconSlot,
            executionBlockNumber: receiptProof.executionBlockNumber,
            executionBlockHash: receiptProof.executionBlockHash,
            executionReceiptsRoot: "0x" + String(repeating: "99", count: 32),
            beaconFinalizedRoot: receiptProof.beaconFinalizedRoot,
            syncCommitteeRoot: receiptProof.syncCommitteeRoot,
            receiptRootIndex: receiptProof.receiptRootIndex,
            receiptTrieProofNodes: receiptProof.receiptTrieProofNodes,
            inclusionBranch: receiptProof.inclusionBranch
        )
        await assertEvmError(.invalidPublicInputs("receiptProof.executionReceiptsRoot")) {
            _ = try await EthereumMainnetSccp(
                inboundProveFunction: { _ in
                    XCTFail("prover callback must not run with drifted receiptProof")
                    return Data([1, 2, 3])
                }
            ).proveInboundToSora(
                EthereumMainnetInboundEvidence(
                    receipt: sourceReceipt,
                    block: block,
                    beaconFinality: beaconFinality,
                    receiptProof: driftedReceiptProof,
                    sourceBridgeEmitterAddress: sourceBridgeAddress
                )
            )
        }

        var missingFinalizedRoot = beaconFinality
        missingFinalizedRoot.removeValue(forKey: "finalizedHeaderRoot")
        await assertEvmError(.invalidPublicInputs("beaconFinality.finalizedHeaderRoot")) {
            _ = try await EthereumMainnetSccp(
                inboundProveFunction: { _ in
                    XCTFail("prover callback must not run without finalized header root")
                    return Data([1, 2, 3])
                }
            ).proveInboundToSora(
                EthereumMainnetInboundEvidence(
                    receipt: sourceReceipt,
                    block: block,
                    beaconFinality: missingFinalizedRoot,
                    receiptProof: receiptProof,
                    sourceBridgeEmitterAddress: sourceBridgeAddress
                )
            )
        }

        var missingSyncCommitteeRoot = beaconFinality
        missingSyncCommitteeRoot.removeValue(forKey: "syncCommitteeRoot")
        await assertEvmError(.invalidPublicInputs("beaconFinality.syncCommitteeRoot")) {
            _ = try await EthereumMainnetSccp(
                inboundProveFunction: { _ in
                    XCTFail("prover callback must not run without sync committee root")
                    return Data([1, 2, 3])
                }
            ).proveInboundToSora(
                EthereumMainnetInboundEvidence(
                    receipt: sourceReceipt,
                    block: block,
                    beaconFinality: missingSyncCommitteeRoot,
                    receiptProof: receiptProof,
                    sourceBridgeEmitterAddress: sourceBridgeAddress
                )
            )
        }

        var missingBeaconSlot = beaconFinality
        missingBeaconSlot.removeValue(forKey: "beaconSlot")
        await assertEvmError(.invalidPublicInputs("beaconFinality.beaconSlot")) {
            _ = try await EthereumMainnetSccp(
                inboundProveFunction: { _ in
                    XCTFail("prover callback must not run without beacon slot")
                    return Data([1, 2, 3])
                }
            ).proveInboundToSora(
                EthereumMainnetInboundEvidence(
                    receipt: sourceReceipt,
                    block: block,
                    beaconFinality: missingBeaconSlot,
                    receiptProof: receiptProof,
                    sourceBridgeEmitterAddress: sourceBridgeAddress
                )
            )
        }

        let driftedFinalizedRootProof = EthereumMainnetReceiptProof(
            sourceEventDigest: receiptProof.sourceEventDigest,
            beaconSlot: receiptProof.beaconSlot,
            executionBlockNumber: receiptProof.executionBlockNumber,
            executionBlockHash: receiptProof.executionBlockHash,
            executionReceiptsRoot: receiptProof.executionReceiptsRoot,
            beaconFinalizedRoot: "0x" + String(repeating: "99", count: 32),
            syncCommitteeRoot: receiptProof.syncCommitteeRoot,
            receiptRootIndex: receiptProof.receiptRootIndex,
            receiptTrieProofNodes: receiptProof.receiptTrieProofNodes,
            inclusionBranch: receiptProof.inclusionBranch
        )
        await assertEvmError(.invalidPublicInputs("receiptProof.beaconFinalizedRoot")) {
            _ = try await EthereumMainnetSccp(
                inboundProveFunction: { _ in
                    XCTFail("prover callback must not run with drifted finalized root")
                    return Data([1, 2, 3])
                }
            ).proveInboundToSora(
                EthereumMainnetInboundEvidence(
                    receipt: sourceReceipt,
                    block: block,
                    beaconFinality: beaconFinality,
                    receiptProof: driftedFinalizedRootProof,
                    sourceBridgeEmitterAddress: sourceBridgeAddress
                )
            )
        }

        let driftedSyncCommitteeRootProof = EthereumMainnetReceiptProof(
            sourceEventDigest: receiptProof.sourceEventDigest,
            beaconSlot: receiptProof.beaconSlot,
            executionBlockNumber: receiptProof.executionBlockNumber,
            executionBlockHash: receiptProof.executionBlockHash,
            executionReceiptsRoot: receiptProof.executionReceiptsRoot,
            beaconFinalizedRoot: receiptProof.beaconFinalizedRoot,
            syncCommitteeRoot: "0x" + String(repeating: "99", count: 32),
            receiptRootIndex: receiptProof.receiptRootIndex,
            receiptTrieProofNodes: receiptProof.receiptTrieProofNodes,
            inclusionBranch: receiptProof.inclusionBranch
        )
        await assertEvmError(.invalidPublicInputs("receiptProof.syncCommitteeRoot")) {
            _ = try await EthereumMainnetSccp(
                inboundProveFunction: { _ in
                    XCTFail("prover callback must not run with drifted sync committee root")
                    return Data([1, 2, 3])
                }
            ).proveInboundToSora(
                EthereumMainnetInboundEvidence(
                    receipt: sourceReceipt,
                    block: block,
                    beaconFinality: beaconFinality,
                    receiptProof: driftedSyncCommitteeRootProof,
                    sourceBridgeEmitterAddress: sourceBridgeAddress
                )
            )
        }

        let driftedBeaconSlotProof = EthereumMainnetReceiptProof(
            sourceEventDigest: receiptProof.sourceEventDigest,
            beaconSlot: receiptProof.beaconSlot + 1,
            executionBlockNumber: receiptProof.executionBlockNumber,
            executionBlockHash: receiptProof.executionBlockHash,
            executionReceiptsRoot: receiptProof.executionReceiptsRoot,
            beaconFinalizedRoot: receiptProof.beaconFinalizedRoot,
            syncCommitteeRoot: receiptProof.syncCommitteeRoot,
            receiptRootIndex: receiptProof.receiptRootIndex,
            receiptTrieProofNodes: receiptProof.receiptTrieProofNodes,
            inclusionBranch: receiptProof.inclusionBranch
        )
        await assertEvmError(.invalidPublicInputs("receiptProof.beaconSlot")) {
            _ = try await EthereumMainnetSccp(
                inboundProveFunction: { _ in
                    XCTFail("prover callback must not run with drifted beacon slot")
                    return Data([1, 2, 3])
                }
            ).proveInboundToSora(
                EthereumMainnetInboundEvidence(
                    receipt: sourceReceipt,
                    block: block,
                    beaconFinality: beaconFinality,
                    receiptProof: driftedBeaconSlotProof,
                    sourceBridgeEmitterAddress: sourceBridgeAddress
                )
            )
        }

        await assertEvmError(.invalidPublicInputs("executionProvider")) {
            _ = try await EthereumMainnetSccp()
                .collectInboundEvidenceFromReceipt(EthereumMainnetInboundEvidence(transactionHash: txHash))
        }

        await assertEvmError(.invalidPublicInputs("eth_chainId")) {
            let nonMainnetProvider = EthereumMainnetExecutionProviderStub(
                chainId: "0x38",
                receipt: receipt,
                block: block
            )
            _ = try await EthereumMainnetSccp(executionProvider: nonMainnetProvider)
                .collectInboundEvidenceFromReceipt(EthereumMainnetInboundEvidence(receipt: receipt))
        }
        await assertEvmError(.invalidPublicInputs("eth_chainId")) {
            let decimalProvider = EthereumMainnetExecutionProviderStub(
                chainId: "1",
                receipt: receipt,
                block: block
            )
            _ = try await EthereumMainnetSccp(executionProvider: decimalProvider)
                .collectInboundEvidenceFromReceipt(EthereumMainnetInboundEvidence(receipt: receipt))
        }
        await assertEvmError(.invalidPublicInputs("eth_chainId")) {
            let leadingZeroProvider = EthereumMainnetExecutionProviderStub(
                chainId: "0x01",
                receipt: receipt,
                block: block
            )
            _ = try await EthereumMainnetSccp(executionProvider: leadingZeroProvider)
                .collectInboundEvidenceFromReceipt(EthereumMainnetInboundEvidence(receipt: receipt))
        }
        await assertEvmError(.invalidPublicInputs("eth_chainId")) {
            let numericProvider = EthereumMainnetExecutionProviderStub(
                chainId: 1,
                receipt: receipt,
                block: block
            )
            _ = try await EthereumMainnetSccp(executionProvider: numericProvider)
                .collectInboundEvidenceFromReceipt(EthereumMainnetInboundEvidence(receipt: receipt))
        }

        var failedReceipt = receipt
        failedReceipt["status"] = "0x0"
        await assertEvmError(.invalidPublicInputs("receipt.status")) {
            _ = try await sdk.collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(receipt: failedReceipt, block: block)
            )
        }

        var missingReceiptBlockNumber = receipt
        missingReceiptBlockNumber.removeValue(forKey: "blockNumber")
        await assertEvmError(.invalidPublicInputs("receipt.blockNumber")) {
            _ = try await sdk.collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(receipt: missingReceiptBlockNumber, block: block)
            )
        }

        var zeroReceiptBlockNumber = receipt
        zeroReceiptBlockNumber["blockNumber"] = "0x0"
        await assertEvmError(.zeroField("receipt.blockNumber")) {
            _ = try await sdk.collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(receipt: zeroReceiptBlockNumber, block: block)
            )
        }

        var driftedReceipt = receipt
        driftedReceipt["transactionHash"] = "0x" + String(repeating: "ab", count: 32)
        await assertEvmError(.invalidPublicInputs("receipt.transactionHash")) {
            _ = try await sdk.collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(
                    transactionHash: txHash,
                    receipt: driftedReceipt,
                    block: block
                )
            )
        }

        var driftedBlock = block
        driftedBlock["hash"] = "0x" + String(repeating: "bc", count: 32)
        await assertEvmError(.invalidPublicInputs("block.hash")) {
            _ = try await sdk.collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(receipt: receipt, block: driftedBlock)
            )
        }

        var missingBlockNumber = block
        missingBlockNumber.removeValue(forKey: "number")
        await assertEvmError(.invalidPublicInputs("block.number")) {
            _ = try await sdk.collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(receipt: receipt, block: missingBlockNumber)
            )
        }

        var zeroBlockNumber = block
        zeroBlockNumber["number"] = "0x0"
        await assertEvmError(.zeroField("block.number")) {
            _ = try await sdk.collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(receipt: receipt, block: zeroBlockNumber)
            )
        }

        var uppercaseReceipt = receipt
        uppercaseReceipt["transactionHash"] = txHash.uppercased()
        await assertEvmError(.invalidPublicInputs("receipt.transactionHash")) {
            _ = try await sdk.collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(receipt: uppercaseReceipt, block: block)
            )
        }

        var driftedFinalityHash = beaconFinality
        driftedFinalityHash["executionBlockHash"] = "0x" + String(repeating: "bc", count: 32)
        await assertEvmError(.invalidPublicInputs("beaconFinality.executionBlockHash")) {
            _ = try await sdk.collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(receipt: receipt, block: block, beaconFinality: driftedFinalityHash)
            )
        }

        var driftedFinalityNumber = beaconFinality
        driftedFinalityNumber["executionBlockNumber"] = "0x1235"
        await assertEvmError(.invalidPublicInputs("beaconFinality.executionBlockNumber")) {
            _ = try await sdk.collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(receipt: receipt, block: block, beaconFinality: driftedFinalityNumber)
            )
        }

        var driftedFinalityReceiptsRoot = beaconFinality
        driftedFinalityReceiptsRoot["executionReceiptsRoot"] = "0x" + String(repeating: "cd", count: 32)
        await assertEvmError(.invalidPublicInputs("beaconFinality.executionReceiptsRoot")) {
            _ = try await sdk.collectInboundEvidenceFromReceipt(
                EthereumMainnetInboundEvidence(receipt: receipt, block: block, beaconFinality: driftedFinalityReceiptsRoot)
            )
        }

        await assertEvmError(.allZeroProof) {
            _ = try await sdk.submitInboundToIroha(Data([0, 0]))
        }
    }

    func testEthereumMainnetInboundProverReceivesCallbackEvidenceSnapshot() async throws {
        let txHash = "0x" + String(repeating: "aa", count: 32)
        let blockHash = "0x" + String(repeating: "bb", count: 32)
        let sourceEventDigest = "0x" + String(repeating: "ee", count: 32)
        let sourceBridgeAddress = "0x" + String(repeating: "44", count: 20)
        let receiptsRoot = "0x" + String(repeating: "cc", count: 32)
        let finalizedRoot = "0x" + String(repeating: "dd", count: 32)
        let syncCommitteeRoot = "0x" + String(repeating: "aa", count: 32)
        let receiptNested = NSMutableDictionary(dictionary: [
            "value": "keep",
            "bytes": NSMutableData(data: Data([0xbb])),
        ])
        let receiptWitness = NSMutableArray(array: [receiptNested])
        let blockWitness = NSMutableDictionary(dictionary: [
            "value": "block",
            "bytes": NSMutableData(data: Data([0xcc])),
        ])
        let finalityBranchWitness = NSMutableArray(array: Self.ethereumFinalityBranch)
        let finalityBytes = NSMutableData(data: Data([0xaa]))
        let finalityWitness = NSMutableDictionary(dictionary: [
            "branch": finalityBranchWitness,
            "bytes": finalityBytes,
        ])
        let blockReceiptsWitness = NSMutableArray(array: ["receipt-list"])
        let sourceEventLog: [String: Any] = [
            "address": sourceBridgeAddress,
            "transactionHash": txHash,
            "blockHash": blockHash,
            "blockNumber": "0x1234",
            "topics": [evmSccpSourceEventTopic(), sourceEventDigest],
            "data": "0x",
        ]
        let receipt: [String: Any] = [
            "transactionHash": txHash,
            "blockHash": blockHash,
            "blockNumber": "0x1234",
            "status": "0x1",
            "logs": [sourceEventLog],
            "mutableWitness": receiptWitness,
        ]
        let block: [String: Any] = [
            "hash": blockHash,
            "number": "0x1234",
            "receiptsRoot": receiptsRoot,
            "mutableWitness": blockWitness,
        ]
        let beaconFinality: [String: Any] = [
            "executionBlockNumber": "0x1234",
            "executionBlockHash": blockHash,
            "executionReceiptsRoot": receiptsRoot,
            "finalizedHeaderRoot": finalizedRoot,
            "syncCommitteeRoot": syncCommitteeRoot,
            "beaconSlot": "0x20",
            "finalityBranch": Self.ethereumFinalityBranch,
            "syncCommitteeBits": Self.ethereumSyncCommitteeSupermajorityBits,
            "syncCommitteeSignature": "0x" + String(repeating: "34", count: 96),
            "syncCommitteeParticipation": Self.ethereumSyncCommitteeSupermajorityParticipation,
            "syncSignatureSlot": "65",
            "mutableWitness": finalityWitness,
        ]
        var blockReceipt = receipt
        blockReceipt["mutableWitness"] = blockReceiptsWitness
        var mutableReceiptProofNode = Data([0x01, 0x02])
        var mutableReceiptProofBranch = Data(repeating: 0x11, count: 32)
        var mutableInputBranch = Data([0x44])
        let receiptProof = EthereumMainnetReceiptProof(
            sourceEventDigest: sourceEventDigest,
            beaconSlot: 32,
            executionBlockNumber: 0x1234,
            executionBlockHash: blockHash,
            executionReceiptsRoot: receiptsRoot,
            beaconFinalizedRoot: finalizedRoot,
            syncCommitteeRoot: syncCommitteeRoot,
            receiptRootIndex: 0,
            receiptTrieProofNodes: [mutableReceiptProofNode],
            inclusionBranch: [mutableReceiptProofBranch]
        )
        let receiptProofHash = try evmSccpReceiptProofHash(
            sourceEventDigest: receiptProof.sourceEventDigest,
            beaconSlot: receiptProof.beaconSlot,
            executionBlockNumber: receiptProof.executionBlockNumber,
            executionBlockHash: receiptProof.executionBlockHash,
            executionReceiptsRoot: receiptProof.executionReceiptsRoot,
            beaconFinalizedRoot: receiptProof.beaconFinalizedRoot,
            syncCommitteeRoot: receiptProof.syncCommitteeRoot,
            receiptRootIndex: receiptProof.receiptRootIndex,
            receiptTrieProofNodes: receiptProof.receiptTrieProofNodes,
            inclusionBranch: receiptProof.inclusionBranch
        )
        let proofBytes = try await EthereumMainnetSccp(
            inboundProveFunction: { evidence in
                receiptWitness.add("changed")
                receiptNested.setObject("changed", forKey: "value" as NSString)
                blockWitness.setObject("changed", forKey: "value" as NSString)
                finalityBranchWitness.add("0x" + String(repeating: "99", count: 32))
                finalityBytes.setData(Data([0xff]))
                finalityWitness.setObject("changed", forKey: "new" as NSString)
                blockReceiptsWitness.add("changed")
                mutableReceiptProofNode[0] = 0xff
                mutableReceiptProofBranch[0] = 0xee
                mutableInputBranch[0] = 0x45

                XCTAssertNil(evidence.receipt?["mutableWitness"] as? NSMutableArray)
                let receiptSnapshot = try XCTUnwrap(evidence.receipt?["mutableWitness"] as? [Any])
                XCTAssertEqual(receiptSnapshot.count, 1)
                let receiptNestedSnapshot = try XCTUnwrap(receiptSnapshot[0] as? [String: Any])
                XCTAssertEqual(receiptNestedSnapshot["value"] as? String, "keep")
                XCTAssertEqual(try XCTUnwrap(receiptNestedSnapshot["bytes"] as? Data), Data([0xbb]))

                XCTAssertNil(evidence.block?["mutableWitness"] as? NSMutableDictionary)
                let blockSnapshot = try XCTUnwrap(evidence.block?["mutableWitness"] as? [String: Any])
                XCTAssertEqual(blockSnapshot["value"] as? String, "block")
                XCTAssertEqual(try XCTUnwrap(blockSnapshot["bytes"] as? Data), Data([0xcc]))

                XCTAssertNil(evidence.beaconFinality?["mutableWitness"] as? NSMutableDictionary)
                let finalitySnapshot = try XCTUnwrap(evidence.beaconFinality?["mutableWitness"] as? [String: Any])
                let branchSnapshot = try XCTUnwrap(finalitySnapshot["branch"] as? [Any])
                XCTAssertEqual(branchSnapshot.count, Self.ethereumFinalityBranch.count)
                XCTAssertEqual(branchSnapshot.first as? String, Self.ethereumFinalityBranch.first)
                XCTAssertEqual(try XCTUnwrap(finalitySnapshot["bytes"] as? Data), Data([0xaa]))

                let blockReceiptsSnapshot = try XCTUnwrap(evidence.blockReceipts)
                XCTAssertNil(blockReceiptsSnapshot[0]["mutableWitness"] as? NSMutableArray)
                let blockReceiptWitnessSnapshot = try XCTUnwrap(blockReceiptsSnapshot[0]["mutableWitness"] as? [Any])
                XCTAssertEqual(blockReceiptWitnessSnapshot.count, 1)
                XCTAssertEqual(blockReceiptWitnessSnapshot[0] as? String, "receipt-list")

                XCTAssertEqual(evidence.inclusionBranch?[0], Data([0x44]))
                XCTAssertEqual(evidence.receiptProof?.receiptTrieProofNodes[0], Data([0x01, 0x02]))
                XCTAssertEqual(evidence.receiptProof?.inclusionBranch[0], Data(repeating: 0x11, count: 32))
                XCTAssertEqual(evidence.receiptProofHash, receiptProofHash)
                return Data([9, 8, 7])
            }
        ).proveInboundToSora(
            EthereumMainnetInboundEvidence(
                receipt: receipt,
                block: block,
                beaconFinality: beaconFinality,
                blockReceipts: [blockReceipt],
                inclusionBranch: [mutableInputBranch],
                receiptProof: receiptProof,
                receiptProofHash: receiptProofHash,
                sourceBridgeEmitterAddress: sourceBridgeAddress
            )
        )
        XCTAssertEqual(proofBytes, Data([9, 8, 7]))
    }

    func testEthereumMainnetCollectInboundEvidenceSnapshotsConsensusBoundary() async throws {
        let txHash = "0x" + String(repeating: "aa", count: 32)
        let blockHash = "0x" + String(repeating: "bb", count: 32)
        let sourceEventDigest = "0x" + String(repeating: "ee", count: 32)
        let sourceBridgeAddress = "0x" + String(repeating: "44", count: 20)
        let receiptsRoot = "0x" + String(repeating: "cc", count: 32)
        let finalizedRoot = "0x" + String(repeating: "dd", count: 32)
        let syncCommitteeRoot = "0x" + String(repeating: "aa", count: 32)
        let receiptNested = NSMutableDictionary(dictionary: [
            "value": "keep",
            "bytes": NSMutableData(data: Data([0xbb])),
        ])
        let receiptWitness = NSMutableArray(array: [receiptNested])
        let blockWitness = NSMutableDictionary(dictionary: [
            "value": "block",
            "bytes": NSMutableData(data: Data([0xcc])),
        ])
        let finalityBranchWitness = NSMutableArray(array: Self.ethereumFinalityBranch)
        let finalityBytes = NSMutableData(data: Data([0xaa]))
        let finalityWitness = NSMutableDictionary(dictionary: [
            "branch": finalityBranchWitness,
            "bytes": finalityBytes,
        ])
        let sourceEventLog: [String: Any] = [
            "address": sourceBridgeAddress,
            "transactionHash": txHash,
            "blockHash": blockHash,
            "blockNumber": "0x1234",
            "topics": [evmSccpSourceEventTopic(), sourceEventDigest],
            "data": "0x",
        ]
        let receipt: [String: Any] = [
            "transactionHash": txHash,
            "blockHash": blockHash,
            "blockNumber": "0x1234",
            "status": "0x1",
            "logs": [sourceEventLog],
            "mutableWitness": receiptWitness,
        ]
        let block: [String: Any] = [
            "hash": blockHash,
            "number": "0x1234",
            "receiptsRoot": receiptsRoot,
            "mutableWitness": blockWitness,
        ]
        let beaconFinality: [String: Any] = [
            "executionBlockNumber": "0x1234",
            "executionBlockHash": blockHash,
            "executionReceiptsRoot": receiptsRoot,
            "finalizedHeaderRoot": finalizedRoot,
            "syncCommitteeRoot": syncCommitteeRoot,
            "beaconSlot": "0x20",
            "finalityBranch": Self.ethereumFinalityBranch,
            "syncCommitteeBits": Self.ethereumSyncCommitteeSupermajorityBits,
            "syncCommitteeSignature": "0x" + String(repeating: "34", count: 96),
            "syncCommitteeParticipation": Self.ethereumSyncCommitteeSupermajorityParticipation,
            "syncSignatureSlot": "65",
            "mutableWitness": finalityWitness,
        ]
        let consensusProvider = EthereumMainnetMutatingConsensusProviderStub(finality: beaconFinality) { receipt, block, transactionHash in
            XCTAssertEqual(transactionHash, txHash)
            XCTAssertNil(receipt?["mutableWitness"] as? NSMutableArray)
            let receiptSnapshot = try XCTUnwrap(receipt?["mutableWitness"] as? [Any])
            let receiptNestedSnapshot = try XCTUnwrap(receiptSnapshot[0] as? [String: Any])
            XCTAssertEqual(receiptNestedSnapshot["value"] as? String, "keep")
            XCTAssertEqual(try XCTUnwrap(receiptNestedSnapshot["bytes"] as? Data), Data([0xbb]))
            XCTAssertNil(block?["mutableWitness"] as? NSMutableDictionary)
            let blockSnapshot = try XCTUnwrap(block?["mutableWitness"] as? [String: Any])
            XCTAssertEqual(blockSnapshot["value"] as? String, "block")
            XCTAssertEqual(try XCTUnwrap(blockSnapshot["bytes"] as? Data), Data([0xcc]))

            receiptWitness.add("changed")
            receiptNested.setObject("changed", forKey: "value" as NSString)
            blockWitness.setObject("changed", forKey: "value" as NSString)
        }

        let evidence = try await EthereumMainnetSccp(
            consensusProvider: consensusProvider,
            sourceBridgeEmitterAddress: sourceBridgeAddress
        ).collectInboundEvidenceFromReceipt(EthereumMainnetInboundEvidence(receipt: receipt, block: block))
        finalityBranchWitness.add("0x" + String(repeating: "99", count: 32))
        finalityBytes.setData(Data([0xff]))
        finalityWitness.setObject("changed", forKey: "new" as NSString)

        XCTAssertEqual(consensusProvider.calls, 1)
        XCTAssertNil(evidence.receipt?["mutableWitness"] as? NSMutableArray)
        let receiptSnapshot = try XCTUnwrap(evidence.receipt?["mutableWitness"] as? [Any])
        XCTAssertEqual(receiptSnapshot.count, 1)
        let receiptNestedSnapshot = try XCTUnwrap(receiptSnapshot[0] as? [String: Any])
        XCTAssertEqual(receiptNestedSnapshot["value"] as? String, "keep")
        XCTAssertEqual(try XCTUnwrap(receiptNestedSnapshot["bytes"] as? Data), Data([0xbb]))
        XCTAssertNil(evidence.block?["mutableWitness"] as? NSMutableDictionary)
        let blockSnapshot = try XCTUnwrap(evidence.block?["mutableWitness"] as? [String: Any])
        XCTAssertEqual(blockSnapshot["value"] as? String, "block")
        XCTAssertEqual(try XCTUnwrap(blockSnapshot["bytes"] as? Data), Data([0xcc]))
        XCTAssertNil(evidence.beaconFinality?["mutableWitness"] as? NSMutableDictionary)
        let finalitySnapshot = try XCTUnwrap(evidence.beaconFinality?["mutableWitness"] as? [String: Any])
        let branchSnapshot = try XCTUnwrap(finalitySnapshot["branch"] as? [Any])
        XCTAssertEqual(branchSnapshot.count, Self.ethereumFinalityBranch.count)
        XCTAssertEqual(branchSnapshot.first as? String, Self.ethereumFinalityBranch.first)
        XCTAssertEqual(try XCTUnwrap(finalitySnapshot["bytes"] as? Data), Data([0xaa]))
        XCTAssertNil(finalitySnapshot["new"])
    }

    func testBscMainnetSccpFacadeRequiresChainId56AndBscTarget() async throws {
        let binding = try sccpBscMainnetDestinationBinding(
            verifierAddress: "0x" + String(repeating: "11", count: 20),
            bridgeAddress: "0x" + String(repeating: "22", count: 20),
            verifierCodeHash: "0x" + String(repeating: "bb", count: 32),
            verifierKeyHash: "0x" + String(repeating: "cc", count: 32)
        )
        XCTAssertEqual(sccpBscMainnetChainId, 56)
        XCTAssertEqual(binding.networkId, sccpBscMainnetNetworkId)
        XCTAssertEqual(binding.targetDomain, sccpDomainBsc)
        XCTAssertEqual(
            binding.hash,
            try sccpBscMainnetDestinationBindingHash(
                verifierAddress: "0x" + String(repeating: "11", count: 20),
                bridgeAddress: "0x" + String(repeating: "22", count: 20),
                verifierCodeHash: "0x" + String(repeating: "bb", count: 32),
                verifierKeyHash: "0x" + String(repeating: "cc", count: 32)
            )
        )

        let input = try EvmSccpProofRequestInput(
            publicInputs: Self.sampleEvmPublicInputs(targetDomain: sccpDomainBsc),
            bundleBytes: Data([5, 6, 7]),
            sourceProofBytes: Data([9, 10]),
            statementHash: String(repeating: "56", count: 32),
            destinationBinding: binding
        )
        let request = try buildBscMainnetSccpDestinationProofRequest(input)
        let proofBytes = Self.sampleGroth16ProofBytes()
        let proofResult = try wrapBscMainnetSccpDestinationProofResult(
            proofBytes: proofBytes,
            request: request
        )
        XCTAssertThrowsError(try wrapBscMainnetSccpDestinationProofResult(
            proofBytes: proofBytes,
            request: EvmSccpProofRequest(
                version: request.version,
                backend: request.backend,
                sourceDomain: request.sourceDomain,
                targetDomain: request.targetDomain,
                publicInputs: request.publicInputs,
                publicInputsBytes: request.publicInputsBytes,
                publicSignalWords: request.publicSignalWords,
                bundleBytes: request.bundleBytes,
                sourceProofBytes: request.sourceProofBytes,
                proofContext: request.proofContext,
                statementHash: request.statementHash,
                destinationBindingHash: "0x" + String(repeating: "99", count: 32),
                requestHash: request.requestHash,
                destinationBinding: request.destinationBinding
            )
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("destinationBindingHash"))
        }
        let submission = try buildBscMainnetSccpDestinationSubmission(
            EvmSccpSubmissionInput(proofResult: proofResult)
        )
        XCTAssertEqual(request.targetDomain, sccpDomainBsc)
        XCTAssertEqual(request.destinationBinding?.networkId, sccpBscMainnetNetworkId)
        XCTAssertEqual(proofResult.destinationBindingHash, binding.hash)
        XCTAssertEqual(submission.targetDomain, sccpDomainBsc)
        XCTAssertEqual(submission.destinationBindingHash, binding.hash)
        let submitFacade = BscMainnetSccp(outboundSubmitFunction: { outboundSubmission in
            XCTAssertEqual(outboundSubmission.targetDomain, sccpDomainBsc)
            XCTAssertEqual(outboundSubmission.proofBytes, proofBytes)
            XCTAssertEqual(outboundSubmission.destinationBindingHash, binding.hash)
            return "bsc-submitted"
        })
        let submitted = try await submitFacade.submitOutboundToBsc(EvmSccpSubmissionInput(proofResult: proofResult))
        XCTAssertEqual(submitted as? String, "bsc-submitted")
        do {
            _ = try await BscMainnetSccp().submitOutboundToBsc(EvmSccpSubmissionInput(proofResult: proofResult))
            XCTFail("BSC outbound submitter must be app-supplied")
        } catch let error as EvmSccpProverError {
            XCTAssertEqual(error, .localProverUnavailable)
        }

        let prover = BscMainnetSccpProver { request in
            XCTAssertEqual(request.targetDomain, sccpDomainBsc)
            XCTAssertEqual(request.destinationBinding?.networkId, sccpBscMainnetNetworkId)
            return proofBytes
        }
        let asyncResult = try await prover.prove(input)
        XCTAssertEqual(asyncResult.destinationBindingHash, binding.hash)

        XCTAssertThrowsError(try sccpBscMainnetDestinationBinding(
            verifierAddress: "0x" + String(repeating: "11", count: 20),
            bridgeAddress: "0x" + String(repeating: "22", count: 20),
            verifierCodeHash: "0x" + String(repeating: "bb", count: 32),
            verifierKeyHash: "0x" + String(repeating: "cc", count: 32),
            networkId: "0x" + String(repeating: "33", count: 32)
        )) { error in
            XCTAssertEqual(error as? SccpSourceProofHashError, .invalidSourceMaterial("networkId"))
        }

        XCTAssertThrowsError(try buildBscMainnetSccpDestinationProofRequest(
            try Self.sampleProductionEvmProofRequestInput()
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("request.targetDomain"))
        }
        XCTAssertThrowsError(try buildBscMainnetSccpDestinationSubmission(EvmSccpSubmissionInput(
            publicInputs: Self.sampleEvmPublicInputs(targetDomain: sccpDomainBsc),
            proofBytes: proofBytes,
            statementHash: String(repeating: "56", count: 32),
            destinationBindingHash: binding.hash
        ))) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("proofResult.destinationBinding"))
        }
    }

    func testBscMainnetSccpBuildsLocalAdmissionSubmission() throws {
        let input = BscMainnetLocalAdmissionSubmissionInput(
            proofBytes: Data([1, 2, 3]),
            publicInputsBytes: Data([4, 5, 6]),
            bundleBytes: Data([7, 8, 9]),
            envelopeBytes: Data([10, 11, 12]),
            statementHash: "0x" + String(repeating: "66", count: 32),
            sourceVerifierMaterialHash: "0x" + String(repeating: "77", count: 32),
            sourceAdapterEngineDeploymentHash: "0x" + String(repeating: "88", count: 32)
        )
        let submission = try buildBscMainnetSccpLocalAdmissionSubmission(input)
        let facadeSubmission = try BscMainnetSccp().buildLocalAdmissionSubmission(input)

        XCTAssertEqual(submission.platformPayload, sccpLocalAdmissionSubmissionKindV1)
        XCTAssertEqual(submission.envelopeEncoding, sccpLocalAdmissionEnvelopeEncodingV1)
        XCTAssertEqual(submission.verifierEntrypoint, sccpLocalAdmissionEntrypointV1)
        XCTAssertEqual(submission.sourceDomain, sccpDomainBsc)
        XCTAssertEqual(submission.targetDomain, sccpDomainSora)
        XCTAssertTrue(submission.arguments.isEmpty)
        XCTAssertEqual(submission.proofBytes, Data([1, 2, 3]))
        XCTAssertEqual(submission.publicInputsBytes, Data([4, 5, 6]))
        XCTAssertEqual(submission.bundleBytes, Data([7, 8, 9]))
        XCTAssertEqual(submission.envelopeBytes, Data([10, 11, 12]))
        XCTAssertEqual(submission.localAdmission.proofBytes, Data([1, 2, 3]))
        XCTAssertEqual(submission.envelopeHex, facadeSubmission.envelopeHex)

        XCTAssertThrowsError(try buildBscMainnetSccpLocalAdmissionSubmission(
            BscMainnetLocalAdmissionSubmissionInput(
                proofBytes: Data([1, 2, 3]),
                publicInputsBytes: Data([4, 5, 6]),
                bundleBytes: Data([7, 8, 9]),
                envelopeBytes: Data([10, 11, 12]),
                statementHash: "0x" + String(repeating: "66", count: 32),
                sourceVerifierMaterialHash: "0x" + String(repeating: "77", count: 32),
                sourceAdapterEngineDeploymentHash: "0x" + String(repeating: "88", count: 32),
                sourceDomain: sccpDomainEthereum
            )
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("BSC -> SORA"))
        }
        XCTAssertThrowsError(try buildBscMainnetSccpLocalAdmissionSubmission(
            BscMainnetLocalAdmissionSubmissionInput(
                proofBytes: Data([0, 0]),
                publicInputsBytes: Data([4, 5, 6]),
                bundleBytes: Data([7, 8, 9]),
                envelopeBytes: Data([10, 11, 12]),
                statementHash: "0x" + String(repeating: "66", count: 32),
                sourceVerifierMaterialHash: "0x" + String(repeating: "77", count: 32),
                sourceAdapterEngineDeploymentHash: "0x" + String(repeating: "88", count: 32)
            )
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .allZeroProof)
        }
        XCTAssertThrowsError(try buildBscMainnetSccpLocalAdmissionSubmission(
            BscMainnetLocalAdmissionSubmissionInput(
                proofBytes: Data([1, 2, 3]),
                publicInputsBytes: Data([4, 5, 6]),
                bundleBytes: Data([7, 8, 9]),
                envelopeBytes: Data(),
                statementHash: "0x" + String(repeating: "66", count: 32),
                sourceVerifierMaterialHash: "0x" + String(repeating: "77", count: 32),
                sourceAdapterEngineDeploymentHash: "0x" + String(repeating: "88", count: 32)
            )
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .emptyProof)
        }
        XCTAssertThrowsError(try buildBscMainnetSccpLocalAdmissionSubmission(
            BscMainnetLocalAdmissionSubmissionInput(
                proofBytes: Data([1, 2, 3]),
                publicInputsBytes: Data([4, 5, 6]),
                bundleBytes: Data([7, 8, 9]),
                envelopeBytes: Data([10, 11, 12]),
                statementHash: "0x" + String(repeating: "66", count: 32),
                sourceVerifierMaterialHash: "0x" + String(repeating: "77", count: 32),
                sourceAdapterEngineDeploymentHash: "0x" + String(repeating: "88", count: 32),
                envelopeEncoding: "abi_tuple_v1"
            )
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("localAdmission.metadata"))
        }
        XCTAssertThrowsError(try buildBscMainnetSccpLocalAdmissionSubmission(
            BscMainnetLocalAdmissionSubmissionInput(
                proofBytes: Data([1, 2, 3]),
                publicInputsBytes: Data([4, 5, 6]),
                bundleBytes: Data([7, 8, 9]),
                envelopeBytes: Data([10, 11, 12]),
                statementHash: "0x" + String(repeating: "66", count: 32),
                sourceVerifierMaterialHash: "0x" + String(repeating: "77", count: 32),
                sourceAdapterEngineDeploymentHash: "0x" + String(repeating: "88", count: 32),
                proofFamily: "debug-proof-family"
            )
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("localAdmission.metadata"))
        }
    }

    func testBscMainnetInboundEvidenceUsesMainnetRpcAndRejectsDrift() async throws {
        func assertEvmError(
            _ expected: EvmSccpProverError,
            _ operation: () async throws -> Void
        ) async {
            do {
                try await operation()
                XCTFail("expected \(expected)")
            } catch {
                XCTAssertEqual(error as? EvmSccpProverError, expected)
            }
        }

        let txHash = "0x" + String(repeating: "aa", count: 32)
        let blockHash = "0x" + String(repeating: "bb", count: 32)
        var receipt: [String: Any] = [
            "transactionHash": txHash,
            "blockHash": blockHash,
            "blockNumber": "0x1234",
            "status": "0x1",
        ]
        let sourceEventDigest = "0x" + String(repeating: "ee", count: 32)
        let sourceBridgeAddress = "0x" + String(repeating: "44", count: 20)
        func sourceEventLog(_ overrides: [String: Any] = [:]) -> [String: Any] {
            var log: [String: Any] = [
                "address": sourceBridgeAddress,
                "transactionHash": txHash,
                "blockHash": blockHash,
                "blockNumber": "0x1234",
                "topics": [evmSccpSourceEventTopic(), sourceEventDigest],
                "data": "0x",
            ]
            for (key, value) in overrides {
                log[key] = value
            }
            return log
        }
        receipt["logs"] = [sourceEventLog()]
        let block: [String: Any] = [
            "hash": blockHash,
            "number": "0x1234",
            "receiptsRoot": "0x" + String(repeating: "cc", count: 32),
        ]
        let receiptsRoot = "0x" + String(repeating: "cc", count: 32)
        let receiptProof = BscMainnetReceiptProof(
            sourceEventDigest: sourceEventDigest,
            validatorEpoch: 36,
            blockNumber: 4660,
            blockHash: blockHash,
            receiptsRoot: receiptsRoot,
            validatorSetHash: "0x" + String(repeating: "ab", count: 32),
            commitSealHash: "0x" + String(repeating: "dd", count: 32),
            receiptRootIndex: 3,
            receiptTrieProofNodes: [Data([0x01]), Data([0x02, 0x03])],
            inclusionBranch: [Data(repeating: 0x11, count: 32)]
        )
        let receiptProofHash = try bscSccpReceiptProofHash(
            sourceEventDigest: receiptProof.sourceEventDigest,
            validatorEpoch: receiptProof.validatorEpoch,
            blockNumber: receiptProof.blockNumber,
            blockHash: receiptProof.blockHash,
            receiptsRoot: receiptProof.receiptsRoot,
            validatorSetHash: receiptProof.validatorSetHash,
            commitSealHash: receiptProof.commitSealHash,
            receiptRootIndex: receiptProof.receiptRootIndex,
            receiptTrieProofNodes: receiptProof.receiptTrieProofNodes,
            inclusionBranch: receiptProof.inclusionBranch
        )
        let parliaFinalityEvidence = BscMainnetParliaFinalityEvidence(
            executionBlockNumber: "0x1234",
            executionBlockHash: blockHash,
            executionReceiptsRoot: receiptsRoot,
            additionalFields: [
                "validatorEpoch": "0x24",
                "validatorSetHash": "0x" + String(repeating: "ab", count: 32),
                "commitSealHash": "0x" + String(repeating: "dd", count: 32),
            ]
        )
        let parliaFinality = parliaFinalityEvidence.dictionary
        let provider = BscMainnetExecutionProviderStub(receipt: receipt, block: block)
        let sdk = BscMainnetSccp(
            executionProvider: provider,
            inboundProveFunction: { evidence in
                XCTAssertEqual(evidence.sourceDomain, sccpDomainBsc)
                XCTAssertEqual(evidence.targetDomain, sccpDomainSora)
                XCTAssertEqual(evidence.transactionHash, txHash)
                XCTAssertEqual(evidence.receiptProofHash, receiptProofHash)
                XCTAssertEqual(evidence.receiptProof?.blockHash, blockHash)
                XCTAssertEqual(evidence.receiptProof?.sourceEventDigest, sourceEventDigest)
                XCTAssertEqual(evidence.sourceEventDigest, sourceEventDigest)
                XCTAssertEqual(evidence.sourceBridgeEmitterAddress, sourceBridgeAddress)
                return Data([1, 2, 3])
            },
            inboundSubmitFunction: { proofBytes in
                XCTAssertEqual(proofBytes, Data([1, 2, 3]))
                return "submitted"
            },
            sourceBridgeEmitterAddress: sourceBridgeAddress
        )

        let evidence = try await sdk.collectInboundEvidenceFromReceipt(
            BscMainnetInboundEvidence(
                transactionHash: txHash,
                parliaFinalityEvidence: parliaFinalityEvidence,
                receiptProof: receiptProof
            )
        )
        XCTAssertEqual(evidence.transactionHash, txHash)
        XCTAssertEqual(evidence.receipt?["status"] as? String, "0x1")
        XCTAssertEqual(evidence.block?["receiptsRoot"] as? String, receiptsRoot)
        XCTAssertEqual(evidence.parliaFinality?["executionBlockNumber"] as? String, "4660")
        XCTAssertEqual(evidence.parliaFinality?["executionBlockHash"] as? String, blockHash)
        XCTAssertEqual(evidence.parliaFinality?["executionReceiptsRoot"] as? String, receiptsRoot)
        XCTAssertEqual(evidence.parliaFinality?["commitSealHash"] as? String, "0x" + String(repeating: "dd", count: 32))
        XCTAssertEqual(evidence.receiptProofHash, receiptProofHash)
        XCTAssertEqual(evidence.receiptProof?.receiptsRoot, receiptsRoot)
        XCTAssertEqual(evidence.sourceEventDigest, sourceEventDigest)
        XCTAssertEqual(evidence.sourceBridgeEmitterAddress, sourceBridgeAddress)
        XCTAssertEqual(provider.calls, ["eth_chainId", "eth_getTransactionReceipt", "eth_getBlockByHash"])
        let consensusProvider = BscMainnetConsensusProviderStub(finality: parliaFinality)
        let providerFinalityEvidence = try await BscMainnetSccp(
            executionProvider: BscMainnetExecutionProviderStub(receipt: receipt, block: block),
            consensusProvider: consensusProvider,
            sourceBridgeEmitterAddress: sourceBridgeAddress
        ).collectInboundEvidenceFromReceipt(BscMainnetInboundEvidence(transactionHash: txHash, receiptProof: receiptProof))
        XCTAssertEqual(providerFinalityEvidence.parliaFinality?["executionBlockHash"] as? String, blockHash)
        XCTAssertEqual(providerFinalityEvidence.receiptProofHash, receiptProofHash)
        XCTAssertEqual(providerFinalityEvidence.sourceEventDigest, sourceEventDigest)
        XCTAssertEqual(consensusProvider.calls, 1)
        let proofBytes = try await sdk.proveInboundToSora(evidence)
        XCTAssertEqual(proofBytes, Data([1, 2, 3]))
        let submitResult = try await sdk.submitInboundToIroha(Data([1, 2, 3]))
        XCTAssertEqual(submitResult as? String, "submitted")

        let receiptProofHashOnlyEvidence = try await BscMainnetSccp().collectInboundEvidenceFromReceipt(
            BscMainnetInboundEvidence(receiptProofHash: receiptProofHash)
        )
        XCTAssertEqual(receiptProofHashOnlyEvidence.receiptProofHash, receiptProofHash)
        XCTAssertNil(receiptProofHashOnlyEvidence.receiptProof)
        await assertEvmError(.invalidPublicInputs("receiptProofHash")) {
            _ = try await BscMainnetSccp().collectInboundEvidenceFromReceipt(
                BscMainnetInboundEvidence(
                    receiptProof: receiptProof,
                    receiptProofHash: "0x" + String(repeating: "99", count: 32)
                )
            )
        }
        await assertEvmError(.invalidPublicInputs("receiptProof.sourceDomain")) {
            _ = try await BscMainnetSccp().collectInboundEvidenceFromReceipt(
                BscMainnetInboundEvidence(
                    receiptProof: BscMainnetReceiptProof(
                        sourceDomain: sccpDomainEthereum,
                        sourceEventDigest: receiptProof.sourceEventDigest,
                        validatorEpoch: receiptProof.validatorEpoch,
                        blockNumber: receiptProof.blockNumber,
                        blockHash: receiptProof.blockHash,
                        receiptsRoot: receiptProof.receiptsRoot,
                        validatorSetHash: receiptProof.validatorSetHash,
                        commitSealHash: receiptProof.commitSealHash,
                        receiptRootIndex: receiptProof.receiptRootIndex,
                        receiptTrieProofNodes: receiptProof.receiptTrieProofNodes,
                        inclusionBranch: receiptProof.inclusionBranch
                    )
                )
            )
        }

        var missingFinalityCallbackCalled = false
        let missingFinalitySdk = BscMainnetSccp(inboundProveFunction: { _ in
            missingFinalityCallbackCalled = true
            return Data([1])
        })
        await assertEvmError(.invalidPublicInputs("parliaFinality")) {
            _ = try await missingFinalitySdk.proveInboundToSora(
                BscMainnetInboundEvidence(receiptProofHash: txHash)
            )
        }
        XCTAssertFalse(missingFinalityCallbackCalled)

        var calledWithHashOnly = false
        await assertEvmError(.invalidPublicInputs("receiptProof")) {
            _ = try await BscMainnetSccp(inboundProveFunction: { _ in
                calledWithHashOnly = true
                return Data([1, 2, 3])
            }).proveInboundToSora(
                BscMainnetInboundEvidence(
                    parliaFinality: parliaFinality,
                    receiptProofHash: receiptProofHash
                )
            )
        }
        XCTAssertFalse(calledWithHashOnly)

        var missingSourceEventCallbackCalled = false
        await assertEvmError(.invalidPublicInputs("receipt.sourceEvent")) {
            _ = try await BscMainnetSccp(inboundProveFunction: { _ in
                missingSourceEventCallbackCalled = true
                return Data([1, 2, 3])
            }).proveInboundToSora(
                BscMainnetInboundEvidence(
                    parliaFinality: parliaFinality,
                    receiptProof: receiptProof
                )
            )
        }
        XCTAssertFalse(missingSourceEventCallbackCalled)

        await assertEvmError(.invalidPublicInputs("receiptProof.receiptsRoot")) {
            _ = try await BscMainnetSccp(inboundProveFunction: { _ in
                XCTFail("prover callback must not run with drifted receipt proof")
                return Data([1, 2, 3])
            }).proveInboundToSora(
                BscMainnetInboundEvidence(
                    receipt: receipt,
                    block: block,
                    parliaFinality: parliaFinality,
                    receiptProof: BscMainnetReceiptProof(
                        sourceEventDigest: receiptProof.sourceEventDigest,
                        validatorEpoch: receiptProof.validatorEpoch,
                        blockNumber: receiptProof.blockNumber,
                        blockHash: receiptProof.blockHash,
                        receiptsRoot: "0x" + String(repeating: "99", count: 32),
                        validatorSetHash: receiptProof.validatorSetHash,
                        commitSealHash: receiptProof.commitSealHash,
                        receiptRootIndex: receiptProof.receiptRootIndex,
                        receiptTrieProofNodes: receiptProof.receiptTrieProofNodes,
                        inclusionBranch: receiptProof.inclusionBranch
                    )
                )
            )
        }

        var driftedSourceReceipt = receipt
        driftedSourceReceipt["logs"] = [
            sourceEventLog(["topics": [evmSccpSourceEventTopic(), "0x" + String(repeating: "99", count: 32)]]),
        ]
        await assertEvmError(.invalidPublicInputs("receiptProof.sourceEventDigest")) {
            _ = try await BscMainnetSccp(
                inboundProveFunction: { _ in
                    XCTFail("prover callback must not run with drifted receipt source event")
                    return Data([1, 2, 3])
                },
                sourceBridgeEmitterAddress: sourceBridgeAddress
            ).proveInboundToSora(
                BscMainnetInboundEvidence(
                    receipt: driftedSourceReceipt,
                    block: block,
                    parliaFinality: parliaFinality,
                    receiptProof: receiptProof
                )
            )
        }

        var extraTopicBscSourceReceipt = receipt
        extraTopicBscSourceReceipt["logs"] = [
            sourceEventLog([
                "topics": [evmSccpSourceEventTopic(), sourceEventDigest, "0x" + String(repeating: "66", count: 32)],
            ]),
        ]
        await assertEvmError(.invalidPublicInputs("receipt.logs[0].topics")) {
            _ = try await BscMainnetSccp(sourceBridgeEmitterAddress: sourceBridgeAddress)
                .collectInboundEvidenceFromReceipt(
                    BscMainnetInboundEvidence(receipt: extraTopicBscSourceReceipt, block: block)
                )
        }

        var nonEmptyDataBscSourceReceipt = receipt
        nonEmptyDataBscSourceReceipt["logs"] = [sourceEventLog(["data": "0x01"])]
        await assertEvmError(.invalidPublicInputs("receipt.logs[0].data")) {
            _ = try await BscMainnetSccp(sourceBridgeEmitterAddress: sourceBridgeAddress)
                .collectInboundEvidenceFromReceipt(
                    BscMainnetInboundEvidence(receipt: nonEmptyDataBscSourceReceipt, block: block)
                )
        }

        var zeroDigestBscSourceReceipt = receipt
        zeroDigestBscSourceReceipt["logs"] = [
            sourceEventLog([
                "topics": [evmSccpSourceEventTopic(), "0x" + String(repeating: "00", count: 32)],
            ]),
        ]
        await assertEvmError(.invalidPublicInputs("receipt.logs[0].topics[1]")) {
            _ = try await BscMainnetSccp(sourceBridgeEmitterAddress: sourceBridgeAddress)
                .collectInboundEvidenceFromReceipt(
                    BscMainnetInboundEvidence(receipt: zeroDigestBscSourceReceipt, block: block)
                )
        }

        var duplicateBscSourceReceipt = receipt
        duplicateBscSourceReceipt["logs"] = [sourceEventLog(), sourceEventLog()]
        await assertEvmError(.invalidPublicInputs("receipt.logs")) {
            _ = try await BscMainnetSccp(sourceBridgeEmitterAddress: sourceBridgeAddress)
                .collectInboundEvidenceFromReceipt(
                    BscMainnetInboundEvidence(receipt: duplicateBscSourceReceipt, block: block)
                )
        }

        var removedBscSourceReceipt = receipt
        removedBscSourceReceipt["logs"] = [sourceEventLog(["removed": true])]
        await assertEvmError(.invalidPublicInputs("receipt.logs")) {
            _ = try await BscMainnetSccp(sourceBridgeEmitterAddress: sourceBridgeAddress)
                .collectInboundEvidenceFromReceipt(
                    BscMainnetInboundEvidence(receipt: removedBscSourceReceipt, block: block)
                )
        }

        var missingBscSourceContextLog = sourceEventLog()
        missingBscSourceContextLog.removeValue(forKey: "transactionHash")
        var missingBscSourceContextReceipt = receipt
        missingBscSourceContextReceipt["logs"] = [missingBscSourceContextLog]
        await assertEvmError(.invalidPublicInputs("receipt.logs[0].transactionHash")) {
            _ = try await BscMainnetSccp(sourceBridgeEmitterAddress: sourceBridgeAddress)
                .collectInboundEvidenceFromReceipt(
                    BscMainnetInboundEvidence(receipt: missingBscSourceContextReceipt, block: block)
                )
        }

        await assertEvmError(.allZeroProof) {
            _ = try await BscMainnetSccp(inboundProveFunction: { _ in Data([0, 0]) })
                .proveInboundToSora(evidence)
        }

        await assertEvmError(.invalidPublicInputs("eth_chainId")) {
            let nonMainnetProvider = BscMainnetExecutionProviderStub(
                chainId: "0x1",
                receipt: receipt,
                block: block
            )
            _ = try await BscMainnetSccp(executionProvider: nonMainnetProvider)
                .collectInboundEvidenceFromReceipt(BscMainnetInboundEvidence(receipt: receipt))
        }
        await assertEvmError(.invalidPublicInputs("eth_chainId")) {
            let decimalProvider = BscMainnetExecutionProviderStub(
                chainId: "56",
                receipt: receipt,
                block: block
            )
            _ = try await BscMainnetSccp(executionProvider: decimalProvider)
                .collectInboundEvidenceFromReceipt(BscMainnetInboundEvidence(receipt: receipt))
        }
        await assertEvmError(.invalidPublicInputs("eth_chainId")) {
            let numericProvider = BscMainnetExecutionProviderStub(
                chainId: 56,
                receipt: receipt,
                block: block
            )
            _ = try await BscMainnetSccp(executionProvider: numericProvider)
                .collectInboundEvidenceFromReceipt(BscMainnetInboundEvidence(receipt: receipt))
        }

        await assertEvmError(.invalidPublicInputs("sourceDomain")) {
            _ = try await sdk.collectInboundEvidenceFromReceipt(
                BscMainnetInboundEvidence(sourceDomain: sccpDomainEthereum, receipt: receipt, block: block)
            )
        }

        var failedReceipt = receipt
        failedReceipt["status"] = "0x0"
        await assertEvmError(.invalidPublicInputs("receipt.status")) {
            _ = try await sdk.collectInboundEvidenceFromReceipt(
                BscMainnetInboundEvidence(receipt: failedReceipt, block: block)
            )
        }

        var missingReceiptBlockNumber = receipt
        missingReceiptBlockNumber.removeValue(forKey: "blockNumber")
        await assertEvmError(.invalidPublicInputs("receipt.blockNumber")) {
            _ = try await sdk.collectInboundEvidenceFromReceipt(
                BscMainnetInboundEvidence(receipt: missingReceiptBlockNumber, block: block)
            )
        }

        var zeroReceiptBlockNumber = receipt
        zeroReceiptBlockNumber["blockNumber"] = "0x0"
        await assertEvmError(.zeroField("receipt.blockNumber")) {
            _ = try await sdk.collectInboundEvidenceFromReceipt(
                BscMainnetInboundEvidence(receipt: zeroReceiptBlockNumber, block: block)
            )
        }

        var driftedReceipt = receipt
        driftedReceipt["transactionHash"] = "0x" + String(repeating: "ab", count: 32)
        await assertEvmError(.invalidPublicInputs("receipt.transactionHash")) {
            _ = try await sdk.collectInboundEvidenceFromReceipt(
                BscMainnetInboundEvidence(
                    transactionHash: txHash,
                    receipt: driftedReceipt,
                    block: block
                )
            )
        }

        var driftedBlock = block
        driftedBlock["hash"] = "0x" + String(repeating: "bc", count: 32)
        await assertEvmError(.invalidPublicInputs("block.hash")) {
            _ = try await sdk.collectInboundEvidenceFromReceipt(
                BscMainnetInboundEvidence(receipt: receipt, block: driftedBlock)
            )
        }

        var missingBlockNumber = block
        missingBlockNumber.removeValue(forKey: "number")
        await assertEvmError(.invalidPublicInputs("block.number")) {
            _ = try await sdk.collectInboundEvidenceFromReceipt(
                BscMainnetInboundEvidence(receipt: receipt, block: missingBlockNumber)
            )
        }

        var zeroBlockNumber = block
        zeroBlockNumber["number"] = "0x0"
        await assertEvmError(.zeroField("block.number")) {
            _ = try await sdk.collectInboundEvidenceFromReceipt(
                BscMainnetInboundEvidence(receipt: receipt, block: zeroBlockNumber)
            )
        }

        var driftedBlockNumber = block
        driftedBlockNumber["number"] = "0x1235"
        await assertEvmError(.invalidPublicInputs("block.number")) {
            _ = try await sdk.collectInboundEvidenceFromReceipt(
                BscMainnetInboundEvidence(receipt: receipt, block: driftedBlockNumber)
            )
        }

        var uppercaseReceipt = receipt
        uppercaseReceipt["transactionHash"] = txHash.uppercased()
        await assertEvmError(.invalidPublicInputs("receipt.transactionHash")) {
            _ = try await sdk.collectInboundEvidenceFromReceipt(
                BscMainnetInboundEvidence(receipt: uppercaseReceipt, block: block)
            )
        }

        var driftedFinalityHash = parliaFinality
        driftedFinalityHash["executionBlockHash"] = "0x" + String(repeating: "bc", count: 32)
        await assertEvmError(.invalidPublicInputs("parliaFinality.executionBlockHash")) {
            _ = try await sdk.collectInboundEvidenceFromReceipt(
                BscMainnetInboundEvidence(receipt: receipt, block: block, parliaFinality: driftedFinalityHash)
            )
        }

        var driftedFinalityNumber = parliaFinality
        driftedFinalityNumber["executionBlockNumber"] = "0x1235"
        await assertEvmError(.invalidPublicInputs("parliaFinality.executionBlockNumber")) {
            _ = try await sdk.collectInboundEvidenceFromReceipt(
                BscMainnetInboundEvidence(receipt: receipt, block: block, parliaFinality: driftedFinalityNumber)
            )
        }

        var driftedFinalityReceiptsRoot = parliaFinality
        driftedFinalityReceiptsRoot["executionReceiptsRoot"] = "0x" + String(repeating: "cd", count: 32)
        await assertEvmError(.invalidPublicInputs("parliaFinality.executionReceiptsRoot")) {
            _ = try await sdk.collectInboundEvidenceFromReceipt(
                BscMainnetInboundEvidence(receipt: receipt, block: block, parliaFinality: driftedFinalityReceiptsRoot)
            )
        }

        await assertEvmError(.allZeroProof) {
            _ = try await sdk.submitInboundToIroha(Data([0, 0]))
        }
    }

    func testBscMainnetCollectInboundEvidenceSnapshotsConsensusBoundary() async throws {
        let txHash = "0x" + String(repeating: "aa", count: 32)
        let blockHash = "0x" + String(repeating: "bb", count: 32)
        let sourceEventDigest = "0x" + String(repeating: "ee", count: 32)
        let sourceBridgeAddress = "0x" + String(repeating: "44", count: 20)
        let receiptsRoot = "0x" + String(repeating: "cc", count: 32)
        let validatorSetHash = "0x" + String(repeating: "ab", count: 32)
        let commitSealHash = "0x" + String(repeating: "dd", count: 32)
        let receiptNested = NSMutableDictionary(dictionary: [
            "value": "keep",
            "bytes": NSMutableData(data: Data([0xbb])),
        ])
        let receiptWitness = NSMutableArray(array: [receiptNested])
        let blockWitness = NSMutableDictionary(dictionary: [
            "value": "block",
            "bytes": NSMutableData(data: Data([0xcc])),
        ])
        let finalityBranchWitness = NSMutableArray(array: [validatorSetHash])
        let finalityBytes = NSMutableData(data: Data([0xaa]))
        let finalityWitness = NSMutableDictionary(dictionary: [
            "branch": finalityBranchWitness,
            "bytes": finalityBytes,
        ])
        let sourceEventLog: [String: Any] = [
            "address": sourceBridgeAddress,
            "transactionHash": txHash,
            "blockHash": blockHash,
            "blockNumber": "0x1234",
            "topics": [evmSccpSourceEventTopic(), sourceEventDigest],
            "data": "0x",
        ]
        let receipt: [String: Any] = [
            "transactionHash": txHash,
            "blockHash": blockHash,
            "blockNumber": "0x1234",
            "status": "0x1",
            "logs": [sourceEventLog],
            "mutableWitness": receiptWitness,
        ]
        let block: [String: Any] = [
            "hash": blockHash,
            "number": "0x1234",
            "receiptsRoot": receiptsRoot,
            "mutableWitness": blockWitness,
        ]
        let parliaFinality: [String: Any] = [
            "executionBlockNumber": "0x1234",
            "executionBlockHash": blockHash,
            "executionReceiptsRoot": receiptsRoot,
            "validatorEpoch": "0x24",
            "validatorSetHash": validatorSetHash,
            "commitSealHash": commitSealHash,
            "mutableWitness": finalityWitness,
        ]
        let consensusProvider = BscMainnetMutatingConsensusProviderStub(finality: parliaFinality) { receipt, block, transactionHash in
            XCTAssertEqual(transactionHash, txHash)
            XCTAssertNil(receipt?["mutableWitness"] as? NSMutableArray)
            let receiptSnapshot = try XCTUnwrap(receipt?["mutableWitness"] as? [Any])
            let receiptNestedSnapshot = try XCTUnwrap(receiptSnapshot[0] as? [String: Any])
            XCTAssertEqual(receiptNestedSnapshot["value"] as? String, "keep")
            XCTAssertEqual(try XCTUnwrap(receiptNestedSnapshot["bytes"] as? Data), Data([0xbb]))
            XCTAssertNil(block?["mutableWitness"] as? NSMutableDictionary)
            let blockSnapshot = try XCTUnwrap(block?["mutableWitness"] as? [String: Any])
            XCTAssertEqual(blockSnapshot["value"] as? String, "block")
            XCTAssertEqual(try XCTUnwrap(blockSnapshot["bytes"] as? Data), Data([0xcc]))

            receiptWitness.add("changed")
            receiptNested.setObject("changed", forKey: "value" as NSString)
            blockWitness.setObject("changed", forKey: "value" as NSString)
        }

        let evidence = try await BscMainnetSccp(
            consensusProvider: consensusProvider,
            sourceBridgeEmitterAddress: sourceBridgeAddress
        ).collectInboundEvidenceFromReceipt(BscMainnetInboundEvidence(receipt: receipt, block: block))
        finalityBranchWitness.add("0x" + String(repeating: "99", count: 32))
        finalityBytes.setData(Data([0xff]))
        finalityWitness.setObject("changed", forKey: "new" as NSString)

        XCTAssertEqual(consensusProvider.calls, 1)
        XCTAssertNil(evidence.receipt?["mutableWitness"] as? NSMutableArray)
        let receiptSnapshot = try XCTUnwrap(evidence.receipt?["mutableWitness"] as? [Any])
        XCTAssertEqual(receiptSnapshot.count, 1)
        let receiptNestedSnapshot = try XCTUnwrap(receiptSnapshot[0] as? [String: Any])
        XCTAssertEqual(receiptNestedSnapshot["value"] as? String, "keep")
        XCTAssertEqual(try XCTUnwrap(receiptNestedSnapshot["bytes"] as? Data), Data([0xbb]))
        XCTAssertNil(evidence.block?["mutableWitness"] as? NSMutableDictionary)
        let blockSnapshot = try XCTUnwrap(evidence.block?["mutableWitness"] as? [String: Any])
        XCTAssertEqual(blockSnapshot["value"] as? String, "block")
        XCTAssertEqual(try XCTUnwrap(blockSnapshot["bytes"] as? Data), Data([0xcc]))
        XCTAssertNil(evidence.parliaFinality?["mutableWitness"] as? NSMutableDictionary)
        let finalitySnapshot = try XCTUnwrap(evidence.parliaFinality?["mutableWitness"] as? [String: Any])
        let branchSnapshot = try XCTUnwrap(finalitySnapshot["branch"] as? [Any])
        XCTAssertEqual(branchSnapshot.count, 1)
        XCTAssertEqual(branchSnapshot.first as? String, validatorSetHash)
        XCTAssertEqual(try XCTUnwrap(finalitySnapshot["bytes"] as? Data), Data([0xaa]))
        XCTAssertNil(finalitySnapshot["new"])
    }

    func testEvmProverRequiresLinkedProofEngine() async throws {
        let prover = EvmSccpProver()

        do {
            _ = try await prover.prove(Self.sampleEvmProofRequestInput())
            XCTFail("expected localProverUnavailable")
        } catch let error as EvmSccpProverError {
            XCTAssertEqual(error, .localProverUnavailable)
        }
    }

    func testEvmProverWrapsExternalProofBytes() async throws {
        let proofBytes = Self.sampleGroth16ProofBytes()
        var seenEvmProofRequests: [EvmSccpProofRequest] = []
        let prover = EvmSccpProver { request in
            seenEvmProofRequests.append(request)
            XCTAssertEqual(request.backend, sccpEvmGroth16Bn254ProofBackendV1)
            XCTAssertEqual(request.targetDomain, sccpDomainEthereum)
            XCTAssertEqual(request.publicSignalWords.count, 9)
            var callbackBundleBytes = request.bundleBytes
            callbackBundleBytes[callbackBundleBytes.startIndex] =
                callbackBundleBytes[callbackBundleBytes.startIndex] == 0 ? 1 : 0
            var callbackSourceProofBytes = request.sourceProofBytes
            if !callbackSourceProofBytes.isEmpty {
                callbackSourceProofBytes[callbackSourceProofBytes.startIndex] =
                    callbackSourceProofBytes[callbackSourceProofBytes.startIndex] == 0 ? 1 : 0
                XCTAssertEqual(request.sourceProofBytes, Data([9, 10]))
                XCTAssertNotEqual(callbackSourceProofBytes, request.sourceProofBytes)
            }
            var callbackPublicSignalWords = request.publicSignalWords
            callbackPublicSignalWords[0] = "0x" + String(repeating: "aa", count: 32)
            XCTAssertEqual(request.bundleBytes, Data([5, 6, 7]))
            XCTAssertNotEqual(callbackBundleBytes, request.bundleBytes)
            XCTAssertEqual(request.publicSignalWords.count, 9)
            XCTAssertNotEqual(callbackPublicSignalWords, request.publicSignalWords)
            return proofBytes
        }

        let sourceProofInput = try Self.sampleProductionEvmProofRequestInput(sourceProofBytes: Data([9, 10]))
        let omittedSourceInput = try Self.sampleProductionEvmProofRequestInput()
        let result = try await prover.prove(sourceProofInput)
        let omittedSourceResult = try await prover.prove(omittedSourceInput)
        XCTAssertTrue(omittedSourceResult.sourceProofBytes.isEmpty)
        XCTAssertEqual(
            seenEvmProofRequests,
            [
                try buildEvmSccpProofRequest(sourceProofInput),
                try buildEvmSccpProofRequest(omittedSourceInput),
            ]
        )

        XCTAssertEqual(result.proofBytes, proofBytes)
        XCTAssertFalse(result.proofBase64.isEmpty)
        XCTAssertEqual(result.statementHash, "0x" + String(repeating: "56", count: 32))
        XCTAssertEqual(result.destinationBindingHash, sourceProofInput.destinationBinding?.hash)
        XCTAssertEqual(result.destinationBinding, sourceProofInput.destinationBinding)
        XCTAssertEqual(result.bundleBytes, Data([5, 6, 7]))
        XCTAssertEqual(result.sourceProofBytes, Data([9, 10]))
        XCTAssertEqual(result.requestHash, try buildEvmSccpProofRequest(sourceProofInput).requestHash)
        XCTAssertEqual(result.envelopeHash.count, 66)
        let artifactInput = try Self.sampleProductionEvmProofRequestInput(
            sourceProofBytes: Data([9, 10]),
            proofArtifactHash: String(repeating: "91", count: 32),
            provingKeyHash: String(repeating: "92", count: 32)
        )
        let artifactRequest = try buildEvmSccpProofRequest(artifactInput)
        let artifactResult = try wrapEvmSccpProofResult(proofBytes: proofBytes, request: artifactRequest)
        XCTAssertEqual(artifactResult.proofArtifactHash, artifactRequest.proofArtifactHash)
        XCTAssertEqual(artifactResult.provingKeyHash, artifactRequest.provingKeyHash)
        XCTAssertNotEqual(artifactRequest.requestHash, try buildEvmSccpProofRequest(sourceProofInput).requestHash)

        let hashOnlyRequest = try buildEvmSccpProofRequest(Self.sampleEvmProofRequestInput(sourceProofBytes: Data([9, 10])))
        XCTAssertThrowsError(try wrapEvmSccpProofResult(
            proofBytes: proofBytes,
            request: hashOnlyRequest
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("request.destinationBinding"))
        }

        let request = try buildEvmSccpProofRequest(sourceProofInput)
        XCTAssertThrowsError(try wrapEvmSccpProofResult(
            proofBytes: Data([0, 0]),
            request: request
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .allZeroProof)
        }
        XCTAssertThrowsError(try wrapEvmSccpProofResult(
            proofBytes: Data([1, 2, 3, 4]),
            request: request
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidProofLength(4))
        }
        let mismatchedRequest = EvmSccpProofRequest(
            version: request.version,
            backend: request.backend,
            sourceDomain: request.sourceDomain,
            targetDomain: request.targetDomain,
            publicInputs: request.publicInputs,
            publicInputsBytes: request.publicInputsBytes,
            publicSignalWords: request.publicSignalWords,
            bundleBytes: request.bundleBytes,
            sourceProofBytes: request.sourceProofBytes,
            proofContext: request.proofContext,
            statementHash: request.statementHash,
            destinationBindingHash: request.destinationBindingHash,
            requestHash: "0x" + String(repeating: "cc", count: 32),
            destinationBinding: request.destinationBinding
        )
        XCTAssertThrowsError(try wrapEvmSccpProofResult(
            proofBytes: proofBytes,
            request: mismatchedRequest
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("request"))
        }
    }

    func testEvmProverResolvesWitnessProviderBeforeBuildingRequest() async throws {
        let proofBytes = Self.sampleGroth16ProofBytes()
        let witnessProvider = EvmSourceProofWitnessProvider()
        let prover = EvmSccpProver(
            witnessProvider: witnessProvider,
            proveFunction: { request in
                XCTAssertEqual(request.sourceProofBytes, Data([9, 10]))
                return proofBytes
            }
        )

        let result = try await prover.prove(try Self.sampleProductionEvmProofRequestInput())

        XCTAssertEqual(witnessProvider.resolveCount, 1)
        XCTAssertEqual(result.sourceProofBytes, Data([9, 10]))
    }

    func testBuildsEvmContractCallSubmission() throws {
        let proofBytes = Self.sampleGroth16ProofBytes()
        let request = try buildEvmSccpProofRequest(try Self.sampleProductionEvmProofRequestInput(
            sourceProofBytes: Data([9, 10])
        ))
        let proofResult = try wrapEvmSccpProofResult(proofBytes: proofBytes, request: request)
        let submission = try buildEvmSccpSubmission(EvmSccpSubmissionInput(proofResult: proofResult))
        let directCallData = try evmSccpSubmitMessageProofCallData(
            proofBytes: proofBytes,
            publicInputs: proofResult.publicInputs,
            statementHash: proofResult.statementHash
        )

        XCTAssertEqual(submission.submissionKind, "contract_call")
        XCTAssertEqual(submission.platformPayload, "evm_groth16_contract_call")
        XCTAssertEqual(submission.envelopeEncoding, sccpEvmContractCallAbiTupleV1)
        XCTAssertEqual(submission.functionSelector, sccpSubmitMessageProofSelectorV1)
        XCTAssertTrue(submission.callDataHex.hasPrefix(sccpSubmitMessageProofSelectorV1))
        XCTAssertEqual(submission.callData.count, 676)
        XCTAssertEqual(
            "0x" + String(repeating: "00", count: 30) + "0100",
            "0x" + submission.callData.subdata(in: 4..<36).hexEncodedString()
        )
        XCTAssertEqual(
            "0x" + String(repeating: "00", count: 30) + "0180",
            "0x" + submission.callData.subdata(in: 260..<292).hexEncodedString()
        )
        XCTAssertEqual(submission.publicInputWords, try evmSccpMessageTransparentPublicInputAbiWords(Self.sampleEvmPublicInputs()))
        XCTAssertEqual(submission.publicSignalWords, proofResult.publicSignalWords)
        XCTAssertEqual(proofResult.bundleBytes, Data([5, 6, 7]))
        XCTAssertEqual(proofResult.sourceProofBytes, Data([9, 10]))
        XCTAssertEqual(proofResult.destinationBinding, request.destinationBinding)
        XCTAssertEqual(submission.envelopeBytes, submission.callData)
        XCTAssertEqual(directCallData, submission.callData)
        let destinationBinding = try sccpEvmDestinationBinding(
            networkId: "0x" + String(repeating: "33", count: 32),
            verifierAddress: "0x" + String(repeating: "11", count: 20),
            bridgeAddress: "0x" + String(repeating: "22", count: 20),
            verifierCodeHash: "0x" + String(repeating: "bb", count: 32),
            verifierKeyHash: "0x" + String(repeating: "cc", count: 32)
        )
        let bindingSubmission = try buildEvmSccpSubmission(try EvmSccpSubmissionInput(
            publicInputs: proofResult.publicInputs,
            proofBytes: proofBytes,
            statementHash: proofResult.statementHash,
            destinationBinding: destinationBinding
        ))
        XCTAssertEqual(bindingSubmission.destinationBindingHash, destinationBinding.hash)

        let omittedSourceProofResult = try wrapEvmSccpProofResult(
            proofBytes: proofBytes,
            request: try buildEvmSccpProofRequest(try Self.sampleProductionEvmProofRequestInput())
        )
        XCTAssertTrue(omittedSourceProofResult.sourceProofBytes.isEmpty)

        var proofMismatch = proofBytes
        proofMismatch[4 * 32 + 31] = 9
        XCTAssertThrowsError(try buildEvmSccpSubmission(EvmSccpSubmissionInput(
            publicInputs: proofResult.publicInputs,
            proofBytes: proofMismatch,
            statementHash: proofResult.statementHash,
            destinationBindingHash: proofResult.destinationBindingHash,
            proofResult: proofResult
        ))) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("proofBytes.a"))
        }
        let bscDestinationBinding = try sccpEvmDestinationBinding(
            targetDomain: sccpDomainBsc,
            networkId: "0x" + String(repeating: "33", count: 32),
            verifierAddress: "0x" + String(repeating: "11", count: 20),
            bridgeAddress: "0x" + String(repeating: "22", count: 20),
            verifierCodeHash: "0x" + String(repeating: "bb", count: 32),
            verifierKeyHash: "0x" + String(repeating: "cc", count: 32)
        )
        XCTAssertThrowsError(try EvmSccpSubmissionInput(
            publicInputs: proofResult.publicInputs,
            proofBytes: proofBytes,
            statementHash: proofResult.statementHash,
            destinationBinding: bscDestinationBinding
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("destinationBinding.targetDomain"))
        }

        let tamperedEnvelopeProofResult = EvmSccpProofResult(
            version: proofResult.version,
            backend: proofResult.backend,
            proofBytes: proofResult.proofBytes,
            proofBase64: proofResult.proofBase64,
            publicInputs: proofResult.publicInputs,
            publicSignalWords: proofResult.publicSignalWords,
            proofContext: proofResult.proofContext,
            statementHash: proofResult.statementHash,
            destinationBindingHash: proofResult.destinationBindingHash,
            requestHash: proofResult.requestHash,
            envelopeHash: "0x" + String(repeating: "aa", count: 32),
            destinationBinding: proofResult.destinationBinding
        )
        XCTAssertThrowsError(try buildEvmSccpSubmission(EvmSccpSubmissionInput(
            proofResult: tamperedEnvelopeProofResult
        ))) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("proofResult.envelopeHash"))
        }

        let tamperedBase64ProofResult = EvmSccpProofResult(
            version: proofResult.version,
            backend: proofResult.backend,
            proofBytes: proofResult.proofBytes,
            proofBase64: "AAAA",
            publicInputs: proofResult.publicInputs,
            publicSignalWords: proofResult.publicSignalWords,
            proofContext: proofResult.proofContext,
            statementHash: proofResult.statementHash,
            destinationBindingHash: proofResult.destinationBindingHash,
            requestHash: proofResult.requestHash,
            envelopeHash: proofResult.envelopeHash,
            destinationBinding: proofResult.destinationBinding
        )
        XCTAssertThrowsError(try buildEvmSccpSubmission(EvmSccpSubmissionInput(
            proofResult: tamperedBase64ProofResult
        ))) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("proofResult.proofBase64"))
        }

        let staleRequestProofResult = EvmSccpProofResult(
            version: proofResult.version,
            backend: proofResult.backend,
            proofBytes: proofResult.proofBytes,
            proofBase64: proofResult.proofBase64,
            publicInputs: proofResult.publicInputs,
            publicSignalWords: proofResult.publicSignalWords,
            bundleBytes: Data([5, 6, 8]),
            sourceProofBytes: proofResult.sourceProofBytes,
            proofContext: proofResult.proofContext,
            statementHash: proofResult.statementHash,
            destinationBindingHash: proofResult.destinationBindingHash,
            requestHash: proofResult.requestHash,
            envelopeHash: proofResult.envelopeHash,
            destinationBinding: proofResult.destinationBinding
        )
        XCTAssertThrowsError(try buildEvmSccpSubmission(EvmSccpSubmissionInput(
            proofResult: staleRequestProofResult
        ))) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("proofResult.requestHash"))
        }

        var mismatchedSignals = proofResult.publicSignalWords
        mismatchedSignals[0] = "0x" + String(repeating: "99", count: 32)
        XCTAssertThrowsError(try buildEvmSccpSubmission(EvmSccpSubmissionInput(
            publicInputs: proofResult.publicInputs,
            proofBytes: proofBytes,
            statementHash: proofResult.statementHash,
            destinationBindingHash: proofResult.destinationBindingHash,
            publicSignalWords: mismatchedSignals
        ))) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("publicSignalWords"))
        }

        XCTAssertThrowsError(try buildEvmSccpSubmission(EvmSccpSubmissionInput(
            publicInputs: Self.sampleEvmPublicInputs(targetDomain: sccpDomainTon),
            proofBytes: proofBytes,
            statementHash: proofResult.statementHash,
            destinationBindingHash: proofResult.destinationBindingHash
        ))) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("publicInputs.targetDomain"))
        }
    }

    func testRejectsMalformedEvmGroth16ProofTuple() throws {
        let request = try buildEvmSccpProofRequest(try Self.sampleProductionEvmProofRequestInput(
            sourceProofBytes: Data([9, 10])
        ))

        XCTAssertThrowsError(try wrapEvmSccpProofResult(
            proofBytes: Self.sampleGroth16ProofBytes(overrides: [0: Self.abiWord(2)]),
            request: request
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("proofBytes.version"))
        }
        XCTAssertThrowsError(try wrapEvmSccpProofResult(
            proofBytes: Self.sampleGroth16ProofBytes(overrides: [4: Data(repeating: 0xff, count: 32)]),
            request: request
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("proofBytes.a.x"))
        }
        XCTAssertThrowsError(try wrapEvmSccpProofResult(
            proofBytes: Self.sampleGroth16ProofBytes(overrides: [
                6: Data(repeating: 0, count: 32),
                7: Data(repeating: 0, count: 32),
                8: Data(repeating: 0, count: 32),
                9: Data(repeating: 0, count: 32),
            ]),
            request: request
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("proofBytes.b"))
        }
        XCTAssertThrowsError(try wrapEvmSccpProofResult(
            proofBytes: Self.sampleGroth16ProofBytes(overrides: [11: Self.abiWord(3)]),
            request: request
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("proofBytes.c"))
        }
        XCTAssertThrowsError(try wrapEvmSccpProofResult(
            proofBytes: Self.sampleGroth16ProofBytes(overrides: [
                6: Self.abiWord(0),
                7: Self.abiWord(1),
                8: Data(hexString: "0cf32d3c49a2cb8a092f24ec3201e68dc299b6216e6321ee60573e3a7f596ea8")!,
                9: Data(hexString: "07bca656753ef8cbee60335acbffe3def91636952d4ab9eb0b839c7f3566c0e2")!,
            ]),
            request: request
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("proofBytes.b"))
        }
        XCTAssertThrowsError(try wrapEvmSccpProofResult(
            proofBytes: Self.sampleGroth16ProofBytes(overrides: [1: Self.repeatedWord(0x22)]),
            request: request
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("proofBytes.messageId"))
        }
        XCTAssertThrowsError(try wrapEvmSccpProofResult(
            proofBytes: Self.sampleGroth16ProofBytes(overrides: [2: Self.abiWord(999)]),
            request: request
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("proofBytes.sourceDomain"))
        }
        XCTAssertThrowsError(try evmSccpSubmitMessageProofCallData(
            proofBytes: Self.sampleGroth16ProofBytes(overrides: [2: Self.abiWord(UInt64(sccpDomainEthereum))]),
            publicInputs: Self.sampleEvmPublicInputs(),
            statementHash: String(repeating: "56", count: 32)
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("proofBytes.sourceDomain"))
        }
        XCTAssertThrowsError(try evmSccpSubmitMessageProofCallData(
            proofBytes: Self.sampleGroth16ProofBytes(overrides: [2: Self.abiWord(UInt64(sccpDomainEthereum))]),
            publicInputs: Self.sampleEvmPublicInputs(),
            statementHash: String(repeating: "56", count: 32),
            sourceDomain: sccpDomainEthereum
        )) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("sourceDomain"))
        }
        XCTAssertThrowsError(try buildEvmSccpSubmission(EvmSccpSubmissionInput(
            publicInputs: Self.sampleEvmPublicInputs(),
            proofBytes: Self.sampleGroth16ProofBytes(overrides: [3: Self.repeatedWord(0x44)]),
            statementHash: String(repeating: "56", count: 32),
            destinationBindingHash: String(repeating: "78", count: 32)
        ))) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("proofBytes.commitmentRoot"))
        }
    }


    func testSccpProofRequestsRejectAllZeroSourceProofBytes() throws {
        let zeroSourceProofBytes = Data([0, 0, 0])
        let oversizedSourceProofBytes = Data(repeating: 1, count: sccpSourceStateMaxProofBytes + 1)

        XCTAssertThrowsError(try buildEvmSccpProofRequest(Self.sampleEvmProofRequestInput(
            sourceProofBytes: zeroSourceProofBytes
        ))) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("sourceProofBytes"))
        }
        XCTAssertThrowsError(try buildEvmSccpProofRequest(Self.sampleEvmProofRequestInput(
            sourceProofBytes: oversizedSourceProofBytes
        ))) { error in
            XCTAssertEqual(error as? EvmSccpProverError, .invalidPublicInputs("sourceProofBytes"))
        }
        XCTAssertThrowsError(try buildTronSccpProofRequest(Self.sampleTronProofRequestInput(
            sourceProofBytes: zeroSourceProofBytes
        ))) { error in
            XCTAssertEqual(error as? TronSccpProverError, .invalidPublicInputs("sourceProofBytes"))
        }
        XCTAssertThrowsError(try buildTronSccpProofRequest(Self.sampleTronProofRequestInput(
            sourceProofBytes: oversizedSourceProofBytes
        ))) { error in
            XCTAssertEqual(error as? TronSccpProverError, .invalidPublicInputs("sourceProofBytes"))
        }
        XCTAssertThrowsError(try buildTonSccpProofRequest(Self.sampleTonProofRequestInput(
            sourceProofBytes: zeroSourceProofBytes
        ))) { error in
            XCTAssertEqual(error as? TonSccpProverError, .invalidField("sourceProofBytes"))
        }

        XCTAssertTrue(try buildEvmSccpProofRequest(Self.sampleEvmProofRequestInput()).sourceProofBytes.isEmpty)
        XCTAssertTrue(try buildTronSccpProofRequest(Self.sampleTronProofRequestInput()).sourceProofBytes.isEmpty)
        XCTAssertTrue(try buildTonSccpProofRequest(Self.sampleTonProofRequestInput()).sourceProofBytes.isEmpty)
    }

    private static func accountsLtHashRequest(
        _ request: SolanaSccpAccountsLtHashProofRequest,
        accountsLtHashProofPublicInputsHash: String? = nil,
        publicInputColumns: [[String]]? = nil,
        fastpqPublicInputs: SolanaSccpAccountsLtHashFastpqPublicInputs? = nil,
        fastpqTransitions: [SolanaSccpAccountsLtHashFastpqTransition]? = nil
    ) -> SolanaSccpAccountsLtHashProofRequest {
        SolanaSccpAccountsLtHashProofRequest(
            version: request.version,
            proofFamily: request.proofFamily,
            circuitId: request.circuitId,
            parameterSet: request.parameterSet,
            sourceDomain: request.sourceDomain,
            finalizedSlot: request.finalizedSlot,
            parentSlot: request.parentSlot,
            sourceStateVerifierId: request.sourceStateVerifierId,
            sourceStateVerifierHash: request.sourceStateVerifierHash,
            accountsLtHashProofPublicInputsHash: accountsLtHashProofPublicInputsHash
                ?? request.accountsLtHashProofPublicInputsHash,
            openedAccountsLtHashContributionsHash: request.openedAccountsLtHashContributionsHash,
            openedAccountsLtHashResidualChecksum: request.openedAccountsLtHashResidualChecksum,
            statementBytes: request.statementBytes,
            accountCommitmentBytes: request.accountCommitmentBytes,
            verificationContextBytes: request.verificationContextBytes,
            schemaDescriptor: request.schemaDescriptor,
            publicInputColumns: publicInputColumns ?? request.publicInputColumns,
            fastpqPublicInputs: fastpqPublicInputs ?? request.fastpqPublicInputs,
            fastpqTransitions: fastpqTransitions ?? request.fastpqTransitions
        )
    }

    private static func fullLightClientAuditRequest(
        _ request: SolanaSccpFullLightClientAuditProofRequest,
        verifierHash: String? = nil,
        auditStatementHash: String? = nil,
        publicInputColumns: [[String]]? = nil,
        fastpqPublicInputs: SolanaSccpFullLightClientAuditFastpqPublicInputs? = nil,
        fastpqTransitions: [SolanaSccpFullLightClientAuditFastpqTransition]? = nil
    ) -> SolanaSccpFullLightClientAuditProofRequest {
        SolanaSccpFullLightClientAuditProofRequest(
            version: request.version,
            proofFamily: request.proofFamily,
            circuitId: request.circuitId,
            parameterSet: request.parameterSet,
            role: request.role,
            roleCode: request.roleCode,
            sourceDomain: request.sourceDomain,
            finalizedSlot: request.finalizedSlot,
            verifierId: request.verifierId,
            verifierHash: verifierHash ?? request.verifierHash,
            sourceStateVerifierId: request.sourceStateVerifierId,
            sourceStateVerifierHash: request.sourceStateVerifierHash,
            sourceVerifierMaterialHash: request.sourceVerifierMaterialHash,
            sourceAdapterDeploymentHash: request.sourceAdapterDeploymentHash,
            fullLightClientGateHash: request.fullLightClientGateHash,
            finalityContextHash: request.finalityContextHash,
            voteMessageHash: request.voteMessageHash,
            accountsLtHashProofHash: request.accountsLtHashProofHash,
            auditStatementHash: auditStatementHash ?? request.auditStatementHash,
            statementBytes: request.statementBytes,
            verificationContextBytes: request.verificationContextBytes,
            schemaDescriptor: request.schemaDescriptor,
            publicInputColumns: publicInputColumns ?? request.publicInputColumns,
            fastpqPublicInputs: fastpqPublicInputs ?? request.fastpqPublicInputs,
            fastpqTransitions: fastpqTransitions ?? request.fastpqTransitions
        )
    }

    private static func sampleProductionWitness(
        mainnetGenesisHash: String = sccpSolanaMainnetGenesisHash,
        sourceStateVerifierHash: String = String(repeating: "ef", count: 32),
        accountsLtHash: Data? = Data((0..<2_048).map { UInt8(($0 % 251) + 1) }),
        destinationBindingHash: String = String(repeating: "78", count: 32)
    ) -> SolanaSccpWitnessInput {
        let branch = [Data(repeating: 0x56, count: 32)]
        let sourceEventDigest = String(repeating: "34", count: 32)
        let blockhash = String(repeating: "9a", count: 32)
        let transactionStatusRoot = try! solanaSccpTransactionStatusRootFromBranch(
            sourceEventDigest: sourceEventDigest,
            transactionSignature: Self.solanaSignature55,
            emitterProgramId: Self.solanaProgram42,
            inclusionBranch: branch
        )
        let messageProofHash = try! solanaSccpMessageProofHash(
            sourceEventDigest: sourceEventDigest,
            transactionStatusRoot: transactionStatusRoot,
            transactionSignature: Self.solanaSignature55,
            emitterProgramId: Self.solanaProgram42,
            inclusionBranch: branch
        )
        let accountsLtHashChecksum = accountsLtHash.map {
            try! solanaSccpAccountsLtHashChecksum($0)
        } ?? String(repeating: "88", count: 32)
        let bankHash = accountsLtHash.map {
            try! solanaSccpAgaveBankHash(
                parentBankHash: String(repeating: "c0", count: 32),
                bankSignatureCount: 8,
                blockhash: blockhash,
                accountsLtHash: $0
            )
        } ?? String(repeating: "aa", count: 32)
        return sampleWitness(
            mainnetGenesisHash: mainnetGenesisHash,
            messageProofHash: messageProofHash,
            inclusionBranch: branch,
            sourceStateVerifierHash: sourceStateVerifierHash,
            sourceAdapterDeploymentHash: String(repeating: "ab", count: 32),
            sourceAdapterDeploymentReceiptHash: String(repeating: "cd", count: 32),
            bankHash: bankHash,
            blockhash: blockhash,
            accountsLtHashChecksum: accountsLtHashChecksum,
            accountsLtHash: accountsLtHash,
            destinationBindingHash: destinationBindingHash
        )
    }

    private static func sampleSolanaRouteCanaryEvidence(
        solanaProgramdataSlot: String = "4321",
        solanaProgramdataExecutableBase64: String = "f0VMRgECAwQF",
        destinationBindingHash: String? = nil,
        expectedDestinationBindingHash: String? = nil
    ) throws -> SolanaSccpRouteCanaryEvidenceInput {
        let canonicalDestinationBindingHash = try sccpDestinationBindingHash(domain: sccpDomainSolana)
        return SolanaSccpRouteCanaryEvidenceInput(
            routeAllowlistHash: "0x" + String(repeating: "31", count: 32),
            destinationBindingHash: destinationBindingHash ?? canonicalDestinationBindingHash,
            expectedDestinationBindingHash: expectedDestinationBindingHash,
            sourceVerifierMaterialHash: "0x" + String(repeating: "33", count: 32),
            sourceAdapterEngineDeploymentHash: "0x" + String(repeating: "34", count: 32),
            verifierIdentity: "3JF3sEqM796hk5WFqA6EtmEwJQ9quALszsfJyvXNQKy3",
            verifierCodeHash: "0xc81178d11a4de525782fe7ac6f5accc2056fa15d1b8c2bfd819eb2ef179c3411",
            solanaProgramAccountDataBase64: "AgAAABERERERERERERERERERERERERERERERERERERERERER",
            solanaProgramdataAddress: "29d2S7vB453rNYFdR5Ycwt7y9haRT5fwVwL9zTmBhfV2",
            solanaProgramdataSlot: solanaProgramdataSlot,
            solanaExpectedProgramdataSlot: "4321",
            solanaProgramAccountContextSlot: "5000",
            solanaProgramdataAccountContextSlot: "5001",
            solanaProgramdataMetadataBlake2b256:
                "0x2b5f26278ea949463e97c1dc5e53a821b82515b405454a1b0e3cd652c3b00209",
            solanaProgramdataMetadataBase64:
                "AwAAAOEQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            solanaProgramdataExecutableBlake2b256:
                "0xc81178d11a4de525782fe7ac6f5accc2056fa15d1b8c2bfd819eb2ef179c3411",
            solanaProgramdataExecutableBase64: solanaProgramdataExecutableBase64
        )
    }

    private static func sampleTonRouteCanaryEvidence(
        destinationBindingHash: String? = nil,
        expectedDestinationBindingHash: String? = nil,
        verifierContractAddress: String = "0:" + String(repeating: "11", count: 32),
        accountStatus: String = "active",
        lastTransactionLt: String = "123456789",
        verifierCodeBocRootHash: String = "0x" + String(repeating: "44", count: 32)
    ) throws -> TonSccpRouteCanaryEvidenceInput {
        let canonicalDestinationBindingHash = try sccpDestinationBindingHash(domain: sccpDomainTon)
        return TonSccpRouteCanaryEvidenceInput(
            routeAllowlistHash: "0x" + String(repeating: "31", count: 32),
            destinationBindingHash: destinationBindingHash ?? canonicalDestinationBindingHash,
            expectedDestinationBindingHash: expectedDestinationBindingHash,
            sourceVerifierMaterialHash: "0x" + String(repeating: "33", count: 32),
            sourceAdapterEngineDeploymentHash: "0x" + String(repeating: "34", count: 32),
            verifierContractAddress: verifierContractAddress,
            verifierCodeHash: "0x" + String(repeating: "44", count: 32),
            accountStatus: accountStatus,
            accountStateHash: "0x" + String(repeating: "55", count: 32),
            lastTransactionLt: lastTransactionLt,
            lastTransactionHash: "0x" + String(repeating: "66", count: 32),
            verifierCodeBocRootHash: verifierCodeBocRootHash
        )
    }

    private static func sampleTronRouteCanaryEvidence(
        routeAllowlistHash: String =
            "0xfea8effb3cddfa458ea79a5a9af6f2d2c33a460b3a66d9305963908c2a3ea67a",
        destinationBindingHash: String? = nil,
        blockNumber: UInt64 = 234,
        usedMessageProof: Bool = true,
        rawDataOwnerMatchesTransaction: Bool = true,
        signatureRecoveredAddress: String =
            "0x417e5f4552091a69125d5dfcb7b8c2659029395bdf",
        signatureRecoversToOwner: Bool = true,
        routeCanaryEvidenceHash: String? =
            "0xe0a96ff7e8f523599fd60fffe8bb3b9fda9519126b7ba00c89c922b323b64e56"
    ) throws -> TronSccpRouteCanaryEvidenceInput {
        let binding = try sccpTronDestinationBinding(
            networkId: "0x" + String(repeating: "33", count: 32),
            verifierAddress: "TJRabPrwbZy45sbavfcjinPJC18kjpRTv8",
            verifierCodeHash: "0x" + String(repeating: "bb", count: 32),
            verifierKeyHash: "0x" + String(repeating: "cc", count: 32)
        )
        return TronSccpRouteCanaryEvidenceInput(
            routeAllowlistHash: routeAllowlistHash,
            destinationBindingHash: destinationBindingHash ?? binding.hash,
            sourceVerifierMaterialHash:
                "0x68c20262e44676bd5f3c4ec428f063373147a1ca14c5885648a9c651b3bcd8d8",
            sourceAdapterEngineDeploymentHash:
                "0x94dbe28a2fb16e043b83639b6dea8ec62f53679599ef1dd220fd13c71c7bdcb8",
            networkId: binding.networkId,
            verifierAddress: binding.verifierAddress,
            verifierCodeHash: binding.verifierCodeHash,
            verifierKeyHash: binding.verifierKeyHash,
            transactionId: "0x" + String(repeating: "fa", count: 32),
            transactionOwnerAddress: "0x417e5f4552091a69125d5dfcb7b8c2659029395bdf",
            blockNumber: blockNumber,
            blockTimestamp: 567000,
            logIndex: 0,
            messageId: "0x" + String(repeating: "dd", count: 32),
            callDataSha256:
                "0xf96dfb36d47a61e7e80df4f19e00b78c12f9a3f3c542e8dac06a7422e1d5f951",
            payloadHash: "0x" + String(repeating: "ab", count: 32),
            commitmentRoot: "0x" + String(repeating: "ee", count: 32),
            finalityHeight: "0x" + String(repeating: "00", count: 31) + "7b",
            finalityBlockHash: "0x" + String(repeating: "cd", count: 32),
            statementHash: "0x" + String(repeating: "f1", count: 32),
            usedMessageProof: usedMessageProof,
            rawDataOwnerMatchesTransaction: rawDataOwnerMatchesTransaction,
            signatureSha256: "0x" + String(repeating: "c4", count: 32),
            signatureRecoveredAddress: signatureRecoveredAddress,
            signatureRecoversToOwner: signatureRecoversToOwner,
            routeCanaryEvidenceHash: routeCanaryEvidenceHash
        )
    }

    private static func sampleWitness(
        targetDomain: UInt32 = sccpDomainSora,
        mainnetGenesisHash: String = sccpSolanaMainnetGenesisHash,
        messageProofHash: String = String(repeating: "cc", count: 32),
        inclusionBranch: [Data] = [],
        sourceStateVerifierId: String = sccpSolanaMainnetAccountsDbVerifierIdV1,
        sourceStateVerifierHash: String = sccpZeroHashV1,
        sourceAdapterDeploymentHash: String = sccpZeroHashV1,
        sourceAdapterDeploymentReceiptHash: String = sccpZeroHashV1,
        bankHash: String = String(repeating: "aa", count: 32),
        blockhash: String = "9xQeWvG816bUx9EPfYdLSdJH7Gq2Xv3yQPG8mD3kAcL7",
        accountsLtHashChecksum: String = String(repeating: "88", count: 32),
        accountsLtHash: Data? = nil,
        transactionSignature: String? = nil,
        emitterProgramId: String? = nil,
        destinationBindingHash: String = String(repeating: "78", count: 32)
    ) -> SolanaSccpWitnessInput {
        let sourceEventDigest = String(repeating: "34", count: 32)
        let transactionSignature = transactionSignature ?? Self.solanaSignature55
        let emitterProgramId = emitterProgramId ?? Self.solanaProgram42
        let transactionStatusRoot = inclusionBranch.isEmpty
            ? String(repeating: "bb", count: 32)
            : try! solanaSccpTransactionStatusRootFromBranch(
                sourceEventDigest: sourceEventDigest,
                transactionSignature: transactionSignature,
                emitterProgramId: emitterProgramId,
                inclusionBranch: inclusionBranch
        )
        return SolanaSccpWitnessInput(
            targetDomain: targetDomain,
            mainnetGenesisHash: mainnetGenesisHash,
            finalizedSlot: 321,
            parentSlot: 320,
            bankSignatureCount: 8,
            parentBankHash: String(repeating: "c0", count: 32),
            blockhash: blockhash,
            bankHash: bankHash,
            transactionStatusRoot: transactionStatusRoot,
            messageProofHash: messageProofHash,
            accountInclusionRoot: String(repeating: "77", count: 32),
            accountsLtHashChecksum: accountsLtHashChecksum,
            accountsLtHash: accountsLtHash,
            transactionSignature: transactionSignature,
            emitterProgramId: emitterProgramId,
            messageId: String(repeating: "dd", count: 32),
            payloadHash: String(repeating: "ee", count: 32),
            commitmentRoot: String(repeating: "12", count: 32),
            sourceEventDigest: sourceEventDigest,
            sourceStateVerifierId: sourceStateVerifierId,
            sourceStateVerifierHash: sourceStateVerifierHash,
            statementHash: String(repeating: "56", count: 32),
            destinationBindingHash: destinationBindingHash,
            inclusionBranch: inclusionBranch,
            sourceAdapterDeploymentHash: sourceAdapterDeploymentHash,
            sourceAdapterDeploymentReceiptHash: sourceAdapterDeploymentReceiptHash
        )
    }

    private static func sampleTonMessageBodyInput(
        publicInputs: TonSccpPublicInputsInput = sampleTonPublicInputs(),
        proofBytes: Data = Data([1, 2, 3, 4]),
        bundleBytes: Data = sampleTonBundleBytes(),
        statementHash: String = String(repeating: "bb", count: 32),
        destinationBindingHash: String = String(repeating: "56", count: 32)
    ) throws -> TonSccpMessageBodyInput {
        let request = try buildTonSccpProofRequest(Self.sampleTonProofRequestInput(
            publicInputs: publicInputs,
            bundleBytes: bundleBytes,
            sourceProofBytes: Data([9, 10]),
            statementHash: statementHash,
            destinationBindingHash: destinationBindingHash,
            sourceAdapterDeploymentHash: String(repeating: "aa", count: 32),
            sourceAdapterDeploymentReceiptHash: String(repeating: "bb", count: 32)
        ))
        let proofResult = try wrapTonSccpProofResult(
            proofBytes: proofBytes,
            request: request
        )
        return try TonSccpMessageBodyInput(
            proofResult: proofResult,
            bundleBytes: bundleBytes,
            metadataBytes: Data([8, 9])
        )
    }

    private static func sampleSolanaSubmissionPublicInputs(
        version: UInt8 = 1,
        targetDomain: UInt32 = sccpDomainSolana
    ) -> SolanaSccpSubmissionPublicInputs {
        SolanaSccpSubmissionPublicInputs(
            version: version,
            messageId: String(repeating: "dd", count: 32),
            payloadHash: String(repeating: "ee", count: 32),
            targetDomain: targetDomain,
            commitmentRoot: String(repeating: "12", count: 32),
            finalityHeight: 321,
            finalityBlockHash: String(repeating: "aa", count: 32)
        )
    }

    private static func sampleSolanaSubmissionPublicInputs(
        from publicInputs: SolanaSccpPublicInputs,
        targetDomain: UInt32 = sccpDomainSolana
    ) -> SolanaSccpSubmissionPublicInputs {
        SolanaSccpSubmissionPublicInputs(
            messageId: publicInputs.messageId,
            payloadHash: publicInputs.payloadHash,
            targetDomain: targetDomain,
            commitmentRoot: publicInputs.commitmentRoot,
            finalityHeight: publicInputs.finalizedSlot,
            finalityBlockHash: publicInputs.bankHash
        )
    }

    private static func sampleTonProofRequestInput(
        publicInputs: TonSccpPublicInputsInput = sampleTonPublicInputs(),
        bundleBytes: Data = sampleTonBundleBytes(),
        sourceProofBytes: Data = Data(),
        statementHash: String = String(repeating: "56", count: 32),
        destinationBindingHash: String = String(repeating: "78", count: 32),
        sourceStateVerifierId: String = sccpTonMainnetShardStateVerifierIdV1,
        sourceStateVerifierHash: String = String(repeating: "cc", count: 32),
        sourceAdapterDeploymentHash: String = String(repeating: "aa", count: 32),
        sourceAdapterDeploymentReceiptHash: String = String(repeating: "bb", count: 32),
        backend: String = sccpTonContractProofBackendV1,
        sourceDomain: UInt32 = sccpDomainTon
    ) -> TonSccpProofRequestInput {
        TonSccpProofRequestInput(
            publicInputs: publicInputs,
            bundleBytes: bundleBytes,
            sourceProofBytes: sourceProofBytes,
            statementHash: statementHash,
            destinationBindingHash: destinationBindingHash,
            sourceStateVerifierId: sourceStateVerifierId,
            sourceStateVerifierHash: sourceStateVerifierHash,
            sourceAdapterDeploymentHash: sourceAdapterDeploymentHash,
            sourceAdapterDeploymentReceiptHash: sourceAdapterDeploymentReceiptHash,
            backend: backend,
            sourceDomain: sourceDomain
        )
    }

    private static func sampleTonShardStateProofRequestInput(
        transactionRoot: String = "0x5a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419",
        sourceStateVerifierHash: String = String(repeating: "d4", count: 32),
        validatorSetTransitionProofs: [TonValidatorSetTransitionProofInput] = []
    ) -> TonShardStateProofRequestInput {
        var shardAccountKey = Data(repeating: 0, count: 32)
        shardAccountKey[0] = 17
        return TonShardStateProofRequestInput(
            masterchainSeqno: 19,
            masterchainBlockHash: String(repeating: "aa", count: 32),
            masterchainFileHash: String(repeating: "a5", count: 32),
            validatorSetHash: "0x68bfccd52bc19cf8cdaffc611d58e53824d0aee395a4d813eca0bcefb3970938",
            masterchainConfigRoot: "0x5bf87008e0e76085d6db977b53a89329de49a4eed8fd1ff90d8c78f096ef05af",
            masterchainConfigProofHash: "0x235c1f0946e38bc210a6a8e193fbe52399ccc4d82693ef3f123be20e27697fc3",
            shardShard: 0x8000_0000_0000_0000,
            shardSeqno: 7,
            shardBlockHash: String(repeating: "bb", count: 32),
            shardFileHash: String(repeating: "bc", count: 32),
            shardStateRoot: "0x12a960855fea2f529c336d7325b1cca784f0f0b1a52ae149d02d046a2499e270",
            transactionRoot: transactionRoot,
            transactionLt: 7,
            shardStateDictionaryRoot: "0x049a63ecefc78dc0cd468ebf47e0385807d790a2ca8e0dca5cbbeb0714567fd3",
            shardStateDictionaryKeyBitLen: 256,
            shardStateDictionaryKey: shardAccountKey,
            masterchainSignatureHash: "0x7a927ad3e689e4f3679fe1d1b8ea1088b914523b0c2da0d6dc0938e5e5cf8d15",
            shardProofHash: "0x32d8b496320e6a1ce5ccf671f2bd6f0d09cb53afed8c123b86cb9327b77c88cf",
            shardStateProofBoc: Data(hexString:
                "b5ee9c720101060100aa00035b9023afe2ffffff110000000000000000000000000000000007000000010000000b000000000000000c000000122001020500000101c00301d37fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff84400000000000000000000000000000000000000000000000000000000000000005a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e419000000000000000780400000000"
            )!,
            shardStateDictionaryProofBoc: Data(hexString:
                "b5ee9c72010103010073000101c00101d37fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff84400000000000000000000000000000000000000000000000000000000000000005a75fc0633903343b684ec73076c5a48cf6b453fc73aa316c2a6de900669e41900000000000000078020000"
            )!,
            configDictionaryProofBoc: Data(hexString:
                "b5ee9c72010106010091000101c00101117fffffff80000008a002012b120000000100000002000200020000000000000003c00302087fff00000405005b14e3a049e28444444444444444444444444444444444444444444444444444444444444444400000000000000060005b14e3a049e288888888888888888888888888888888888888888888888888888888888888888000000000000000a0"
            )!,
            validatorSetTransitionProofs: validatorSetTransitionProofs,
            sourceStateVerifierHash: sourceStateVerifierHash,
            sourceTrustAnchorHash: "0x68bfccd52bc19cf8cdaffc611d58e53824d0aee395a4d813eca0bcefb3970938",
            consensusVerifierHash: String(repeating: "b2", count: 32),
            messageInclusionVerifierHash: String(repeating: "c3", count: 32),
            finalityPolicyHash: String(repeating: "c4", count: 32)
        )
    }

    private static func sampleTonValidatorSetTransitionProofInput(
        signatures: [Data] = [Data(repeating: 0xab, count: 64), Data(repeating: 0xcd, count: 64)]
    ) throws -> TonValidatorSetTransitionProofInput {
        let transitionMessageHash = "0x91eda926884eb1ae700e7b398c46f6d47fbb973efa322564894936140ccd2a19"
        let nextValidatorSetPayload = Data(hexString:
            "0102000000" + String(repeating: "33", count: 32)
                + "0300000000000000" + String(repeating: "44", count: 32)
                + "0400000000000000"
        )!
        let validatorSignatureProof = TonValidatorSignatureProofInput(
            totalWeight: 3,
            signedWeight: 3,
            blockMessageHash: transitionMessageHash,
            validatorPublicKeys: [
                Data(repeating: 0x11, count: 32),
                Data(repeating: 0x22, count: 32)
            ],
            validatorWeights: [1, 2],
            signersBitmap: Data([0x03]),
            signatures: signatures
        )
        return TonValidatorSetTransitionProofInput(
            fromValidatorSetSeqno: 7,
            toValidatorSetSeqno: 8,
            masterchainSeqno: 19,
            masterchainBlockHash: String(repeating: "aa", count: 32),
            masterchainFileHash: String(repeating: "a5", count: 32),
            parentValidatorSetHash: "0x68bfccd52bc19cf8cdaffc611d58e53824d0aee395a4d813eca0bcefb3970938",
            nextValidatorSetHash: "0x26bfcffe8913e5e4f09e56076d5a237cbc5b890d31b8912bd7eacc5d3805691f",
            nextValidatorSetPayload: nextValidatorSetPayload,
            nextValidatorSetPayloadHash: "0xb76b843e99596a049425653e9921e4227af23a5b70331940fa057f1f58314983",
            nextValidatorSetConfigHash: String(repeating: "cc", count: 32),
            transitionMessageHash: transitionMessageHash,
            transitionSignatureHash: try tonValidatorSetTransitionSignatureHash(
                sourceDomain: sccpDomainTon,
                fromValidatorSetSeqno: 7,
                toValidatorSetSeqno: 8,
                masterchainSeqno: 19,
                masterchainWorkchainId: -1,
                masterchainShard: 0x8000_0000_0000_0000,
                masterchainBlockHash: String(repeating: "aa", count: 32),
                masterchainFileHash: String(repeating: "a5", count: 32),
                parentValidatorSetHash: "0x68bfccd52bc19cf8cdaffc611d58e53824d0aee395a4d813eca0bcefb3970938",
                nextValidatorSetHash: "0x26bfcffe8913e5e4f09e56076d5a237cbc5b890d31b8912bd7eacc5d3805691f",
                nextValidatorSetPayload: nextValidatorSetPayload,
                nextValidatorSetPayloadHash: "0xb76b843e99596a049425653e9921e4227af23a5b70331940fa057f1f58314983",
                nextValidatorSetConfigHash: String(repeating: "cc", count: 32),
                transitionMessageHash: transitionMessageHash,
                validatorSignatureProof: validatorSignatureProof
            ),
            validatorSignatureProof: validatorSignatureProof
        )
    }

    private static func sampleTonFullLightClientAuditProofInput(
        masterchainConfigVerifierHash: String = "0x" + String(repeating: "b1", count: 32),
        validatorSetTransitionVerifierHash: String = "0x" + String(repeating: "c2", count: 32),
        shardAccountsDictionaryVerifierHash: String = "0x" + String(repeating: "d3", count: 32),
        shardStateVerificationProofHash: String? = nil,
        masterchainConfigProofHash: String? = nil
    ) throws -> TonSccpFullLightClientAuditProofInput {
        let baseShardState = sampleTonShardStateProofRequestInput()
        let validatorSetPayloadHash = "0xb322afe2faa070a2ed88a922c5ac5d27e5f9fecc41a11ffbed37cca293c4aeb0"
        let configLeafHash = try tonMasterchainConfigLeafHash(
            sourceDomain: sccpDomainTon,
            masterchainSeqno: baseShardState.masterchainSeqno,
            masterchainBlockHash: baseShardState.masterchainBlockHash,
            shardStateRoot: baseShardState.shardStateRoot,
            validatorSetHash: baseShardState.validatorSetHash,
            validatorSetPayloadHash: validatorSetPayloadHash
        )
        let configValueHash = "0x1aa64eb5ca0b3cb254dfada709904ce81f8b327eed0d83f2522122a0a9dddd50"
        let shardState = TonShardStateProofRequestInput(
            masterchainSeqno: baseShardState.masterchainSeqno,
            masterchainWorkchainId: baseShardState.masterchainWorkchainId,
            masterchainShard: baseShardState.masterchainShard,
            masterchainBlockHash: baseShardState.masterchainBlockHash,
            masterchainFileHash: baseShardState.masterchainFileHash,
            validatorSetHash: baseShardState.validatorSetHash,
            masterchainConfigRoot: baseShardState.masterchainConfigRoot,
            masterchainConfigProofHash: masterchainConfigProofHash ?? baseShardState.masterchainConfigProofHash,
            shardWorkchainId: baseShardState.shardWorkchainId,
            shardShard: baseShardState.shardShard,
            shardSeqno: baseShardState.shardSeqno,
            shardBlockHash: baseShardState.shardBlockHash,
            shardFileHash: baseShardState.shardFileHash,
            shardStateRoot: baseShardState.shardStateRoot,
            transactionRoot: baseShardState.transactionRoot,
            transactionLt: baseShardState.transactionLt,
            shardStateDictionaryRoot: baseShardState.shardStateDictionaryRoot,
            shardStateDictionaryKeyBitLen: baseShardState.shardStateDictionaryKeyBitLen,
            shardStateDictionaryKey: baseShardState.shardStateDictionaryKey,
            masterchainSignatureHash: baseShardState.masterchainSignatureHash,
            shardProofHash: baseShardState.shardProofHash,
            shardStateProofBoc: baseShardState.shardStateProofBoc,
            shardStateDictionaryProofBoc: baseShardState.shardStateDictionaryProofBoc,
            configDictionaryProofBoc: baseShardState.configDictionaryProofBoc,
            validatorSetTransitionProofs: baseShardState.validatorSetTransitionProofs,
            sourceStateVerifierId: baseShardState.sourceStateVerifierId,
            sourceStateVerifierHash: baseShardState.sourceStateVerifierHash,
            sourceTrustAnchorId: baseShardState.sourceTrustAnchorId,
            sourceTrustAnchorHash: baseShardState.sourceTrustAnchorHash,
            consensusVerifierId: baseShardState.consensusVerifierId,
            consensusVerifierHash: baseShardState.consensusVerifierHash,
            messageInclusionVerifierId: baseShardState.messageInclusionVerifierId,
            messageInclusionVerifierHash: baseShardState.messageInclusionVerifierHash,
            finalityPolicyId: baseShardState.finalityPolicyId,
            finalityPolicyHash: baseShardState.finalityPolicyHash
        )
        let sourceVerifierMaterialHash = try sccpSourceVerifierMaterialHash(
            sourceDomain: sccpDomainTon,
            sourceTrustAnchorHash: shardState.sourceTrustAnchorHash,
            consensusVerifierHash: shardState.consensusVerifierHash,
            messageInclusionVerifierHash: shardState.messageInclusionVerifierHash,
            finalityPolicyHash: shardState.finalityPolicyHash,
            sourceStateVerifierHash: shardState.sourceStateVerifierHash
        )
        let deploymentReceiptHash = "0x" + String(repeating: "aa", count: 32)
        let sourceAdapterDeploymentHash = try sccpSourceAdapterEngineDeploymentHash(
            sourceDomain: sccpDomainTon,
            sourceTrustAnchorHash: shardState.sourceTrustAnchorHash,
            consensusVerifierHash: shardState.consensusVerifierHash,
            messageInclusionVerifierHash: shardState.messageInclusionVerifierHash,
            finalityPolicyHash: shardState.finalityPolicyHash,
            deploymentReceiptHash: deploymentReceiptHash,
            sourceStateVerifierHash: shardState.sourceStateVerifierHash,
            tonMasterchainConfigVerifierHash: masterchainConfigVerifierHash,
            tonValidatorSetTransitionVerifierHash: validatorSetTransitionVerifierHash,
            tonShardAccountsDictionaryVerifierHash: shardAccountsDictionaryVerifierHash
        )
        let gateHash = try sccpTonFullLightClientGateHash(
            sourceTrustAnchorHash: shardState.sourceTrustAnchorHash,
            consensusVerifierHash: shardState.consensusVerifierHash,
            messageInclusionVerifierHash: shardState.messageInclusionVerifierHash,
            finalityPolicyHash: shardState.finalityPolicyHash,
            deploymentReceiptHash: deploymentReceiptHash,
            tonMasterchainConfigVerifierHash: masterchainConfigVerifierHash,
            tonValidatorSetTransitionVerifierHash: validatorSetTransitionVerifierHash,
            tonShardAccountsDictionaryVerifierHash: shardAccountsDictionaryVerifierHash,
            sourceStateVerifierHash: shardState.sourceStateVerifierHash
        )
        return TonSccpFullLightClientAuditProofInput(
            shardState: shardState,
            shardStateVerificationProof: TonSccpSourceStateVerificationProof(proofBytes: Data([0x11, 0x22, 0x33, 0x44])),
            validatorSetPayloadHash: validatorSetPayloadHash,
            configLeafHash: configLeafHash,
            configValueHash: configValueHash,
            sourceVerifierMaterialHash: sourceVerifierMaterialHash,
            sourceAdapterDeploymentHash: sourceAdapterDeploymentHash,
            fullLightClientGateHash: gateHash,
            tonMasterchainConfigVerifierHash: masterchainConfigVerifierHash,
            tonValidatorSetTransitionVerifierHash: validatorSetTransitionVerifierHash,
            tonShardAccountsDictionaryVerifierHash: shardAccountsDictionaryVerifierHash,
            shardStateVerificationProofHash: shardStateVerificationProofHash
        )
    }

    private static func sampleTonPublicInputs(
        payloadHash: String? = nil,
        targetDomain: UInt32 = sccpDomainTon
    ) -> TonSccpPublicInputsInput {
        let publicInputs = sampleTonBundleFixture().publicInputs
        return TonSccpPublicInputsInput(
            messageId: publicInputs.messageId,
            payloadHash: payloadHash ?? publicInputs.payloadHash,
            targetDomain: targetDomain,
            commitmentRoot: publicInputs.commitmentRoot,
            finalityHeight: publicInputs.finalityHeight,
            finalityBlockHash: publicInputs.finalityBlockHash
        )
    }

    private struct SampleTonBundleFixture {
        let publicInputs: TonSccpPublicInputsInput
        let bundleBytes: Data
    }

    private struct TestTonBundleVecRange {
        let lengthOffset: Int
        let bytesStart: Int
        let bytesEnd: Int
        let bytes: Data
        let nextOffset: Int
    }

    private static func sampleTonBundleBytes(
        sourceDomain: UInt32 = sccpDomainSora,
        senderCodec: UInt8 = 1,
        sender: String = "alice@sora",
        nonce: UInt64 = 327,
        amount: UInt64 = 42,
        routeId: String = "sccp-ton-proof-request",
        merkleProofSteps: [(Data, UInt8)] = [],
        finalityProof: Data = Data([0x71, 0x72])
    ) -> Data {
        sampleTonBundleFixture(
            sourceDomain: sourceDomain,
            senderCodec: senderCodec,
            sender: sender,
            nonce: nonce,
            amount: amount,
            routeId: routeId,
            merkleProofSteps: merkleProofSteps,
            finalityProof: finalityProof
        ).bundleBytes
    }

    private static func sampleTonBundleFixture(
        sourceDomain: UInt32 = sccpDomainSora,
        senderCodec: UInt8 = 1,
        sender: String = "alice@sora",
        nonce: UInt64 = 327,
        amount: UInt64 = 42,
        routeId: String = "sccp-ton-proof-request",
        merkleProofSteps: [(Data, UInt8)] = [],
        finalityProof: Data = Data([0x71, 0x72])
    ) -> SampleTonBundleFixture {
        var payloadBody = Data()
        payloadBody.append(1)
        appendTestU32Le(sourceDomain, to: &payloadBody)
        appendTestU32Le(sccpDomainTon, to: &payloadBody)
        appendTestU64Le(nonce, to: &payloadBody)
        appendTestU32Le(sccpDomainSora, to: &payloadBody)
        payloadBody.append(1)
        appendTestBytes(Data("xor#ton".utf8), to: &payloadBody)
        appendTestU128Le(amount, to: &payloadBody)
        payloadBody.append(senderCodec)
        appendTestBytes(Data(sender.utf8), to: &payloadBody)
        payloadBody.append(4)
        appendTestBytes(Data(("0:" + String(repeating: "12", count: 32)).utf8), to: &payloadBody)
        payloadBody.append(1)
        appendTestBytes(Data(routeId.utf8), to: &payloadBody)

        var payloadBytes = Data([0x02])
        payloadBytes.append(payloadBody)
        var messageIdPreimage = Data("sccp:transfer:v1".utf8)
        messageIdPreimage.append(payloadBody)
        let messageId = "0x" + irohaKeccak256(messageIdPreimage).hexEncodedString()
        var payloadHashPreimage = Data("sccp:payload:v1".utf8)
        payloadHashPreimage.append(payloadBytes)
        let payloadHash = "0x" + Blake2b.hash256(payloadHashPreimage).hexEncodedString()

        var commitmentBytes = Data()
        commitmentBytes.append(1)
        commitmentBytes.append(6)
        appendTestU32Le(sccpDomainTon, to: &commitmentBytes)
        commitmentBytes.append(Data(hexString: String(messageId.dropFirst(2)))!)
        commitmentBytes.append(Data(hexString: String(payloadHash.dropFirst(2)))!)

        var leafPreimage = Data("sccp:hub:leaf:v1".utf8)
        leafPreimage.append(commitmentBytes)
        var currentRoot = Blake2b.hash256(leafPreimage)
        var merkleProof = Data()
        appendTestU32Le(UInt32(merkleProofSteps.count), to: &merkleProof)
        for (sibling, siblingIsLeft) in merkleProofSteps {
            precondition(sibling.count == 32)
            merkleProof.append(sibling)
            merkleProof.append(siblingIsLeft)
            var nodePayload = Data()
            if siblingIsLeft == 1 {
                nodePayload.append(sibling)
                nodePayload.append(currentRoot)
            } else {
                nodePayload.append(currentRoot)
                nodePayload.append(sibling)
            }
            var nodePreimage = Data("sccp:hub:node:v1".utf8)
            nodePreimage.append(nodePayload)
            currentRoot = Blake2b.hash256(nodePreimage)
        }
        let commitmentRoot = "0x" + currentRoot.hexEncodedString()

        var bundle = Data()
        bundle.append(1)
        bundle.append(currentRoot)
        appendTestBytes(commitmentBytes, to: &bundle)
        appendTestBytes(merkleProof, to: &bundle)
        appendTestBytes(payloadBytes, to: &bundle)
        appendTestBytes(finalityProof, to: &bundle)

        return SampleTonBundleFixture(
            publicInputs: TonSccpPublicInputsInput(
                messageId: messageId,
                payloadHash: payloadHash,
                targetDomain: sccpDomainTon,
                commitmentRoot: commitmentRoot,
                finalityHeight: 19,
                finalityBlockHash: String(repeating: "aa", count: 32)
            ),
            bundleBytes: bundle
        )
    }

    private static func splitTestTonSccpMessageProofBundleBytes(_ bundleBytes: Data)
        -> [String: TestTonBundleVecRange] {
        var offset = 33
        let commitment = readTestTonCanonicalVecRange(bundleBytes, offset: offset)
        offset = commitment.nextOffset
        let merkleProof = readTestTonCanonicalVecRange(bundleBytes, offset: offset)
        offset = merkleProof.nextOffset
        let payload = readTestTonCanonicalVecRange(bundleBytes, offset: offset)
        offset = payload.nextOffset
        let finalityProof = readTestTonCanonicalVecRange(bundleBytes, offset: offset)
        return [
            "commitment": commitment,
            "merkleProof": merkleProof,
            "payload": payload,
            "finalityProof": finalityProof,
        ]
    }

    private static func readTestTonCanonicalVecRange(_ bundleBytes: Data, offset: Int) -> TestTonBundleVecRange {
        let length = Int(readTestU32Le(bundleBytes, offset: offset))
        let start = offset + 4
        let end = start + length
        precondition(end <= bundleBytes.count)
        return TestTonBundleVecRange(
            lengthOffset: offset,
            bytesStart: start,
            bytesEnd: end,
            bytes: Data(bundleBytes[start..<end]),
            nextOffset: end
        )
    }

    private static func replaceTestTonSccpMessageProofBundleVec(
        _ bundleBytes: Data,
        range: TestTonBundleVecRange,
        replacement: Data
    ) -> Data {
        var out = Data(bundleBytes[..<range.lengthOffset])
        appendTestU32Le(UInt32(replacement.count), to: &out)
        out.append(replacement)
        out.append(Data(bundleBytes[range.bytesEnd...]))
        return out
    }

    private static func appendTestBytes(_ value: Data, to out: inout Data) {
        appendTestU32Le(UInt32(value.count), to: &out)
        out.append(value)
    }

    private static func appendTestU32Le(_ value: UInt32, to out: inout Data) {
        out.append(UInt8(value & 0xff))
        out.append(UInt8((value >> 8) & 0xff))
        out.append(UInt8((value >> 16) & 0xff))
        out.append(UInt8((value >> 24) & 0xff))
    }

    private static func appendTestU64Le(_ value: UInt64, to out: inout Data) {
        for shift in stride(from: 0, through: 56, by: 8) {
            out.append(UInt8((value >> UInt64(shift)) & 0xff))
        }
    }

    private static func appendTestU128Le(_ value: UInt64, to out: inout Data) {
        appendTestU64Le(value, to: &out)
        appendTestU64Le(0, to: &out)
    }

    private static func readTestU32Le(_ data: Data, offset: Int) -> UInt32 {
        var value: UInt32 = 0
        for index in 0..<4 {
            value |= UInt32(data[offset + index]) << UInt32(index * 8)
        }
        return value
    }

    private static func sampleGroth16ProofBytes(overrides: [Int: Data] = [:]) -> Data {
        var words = [
            abiWord(1),
            repeatedWord(0x11),
            abiWord(UInt64(sccpDomainSora)),
            repeatedWord(0x33),
            abiWord(1),
            abiWord(2),
            bn254G2GeneratorWords[0],
            bn254G2GeneratorWords[1],
            bn254G2GeneratorWords[2],
            bn254G2GeneratorWords[3],
            abiWord(1),
            abiWord(2),
        ]
        for (index, word) in overrides {
            words[index] = word
        }
        var out = Data()
        for word in words {
            out.append(word)
        }
        return out
    }

    private static let bn254G2GeneratorWords = [
        Data(hexString: "1800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6ed")!,
        Data(hexString: "198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c2")!,
        Data(hexString: "12c85ea5db8c6deb4aab71808dcb408fe3d1e7690c43d37b4ce6cc0166fa7daa")!,
        Data(hexString: "090689d0585ff075ec9e99ad690c3395bc4b313370b38ef355acdadcd122975b")!,
    ]

    private static func abiWord(_ value: UInt64) -> Data {
        var out = Data(repeating: 0, count: 32)
        var working = value
        for index in stride(from: 31, through: 0, by: -1) {
            out[index] = UInt8(working & 0xff)
            working >>= 8
            if working == 0 {
                break
            }
        }
        return out
    }

    private static func repeatedWord(_ value: UInt8) -> Data {
        Data(repeating: value, count: 32)
    }

    private static func sampleTronProofRequestInput(
        publicInputs: TronSccpPublicInputsInput = sampleTronPublicInputs(),
        bundleBytes: Data = Data([5, 6, 7]),
        sourceProofBytes: Data = Data(),
        statementHash: String = String(repeating: "56", count: 32),
        destinationBindingHash: String = String(repeating: "78", count: 32),
        backend: String = sccpTronGroth16Bn254ProofBackendV1,
        sourceDomain: UInt32 = sccpDomainSora
    ) -> TronSccpProofRequestInput {
        TronSccpProofRequestInput(
            publicInputs: publicInputs,
            bundleBytes: bundleBytes,
            sourceProofBytes: sourceProofBytes,
            statementHash: statementHash,
            destinationBindingHash: destinationBindingHash,
            backend: backend,
            sourceDomain: sourceDomain
        )
    }

    private static func sampleTronDestinationBinding(
        targetDomain: UInt32 = sccpDomainTron,
        networkId: String = "0x" + String(repeating: "33", count: 32),
        verifierAddress: String = "TJRabPrwbZy45sbavfcjinPJC18kjpRTv8",
        verifierCodeHash: String = "0x" + String(repeating: "bb", count: 32),
        verifierKeyHash: String = "0x" + String(repeating: "cc", count: 32)
    ) throws -> TronSccpDestinationBinding {
        try sccpTronDestinationBinding(
            targetDomain: targetDomain,
            networkId: networkId,
            verifierAddress: verifierAddress,
            verifierCodeHash: verifierCodeHash,
            verifierKeyHash: verifierKeyHash
        )
    }

    private static func sampleProductionTronProofRequestInput(
        publicInputs: TronSccpPublicInputsInput = sampleTronPublicInputs(),
        bundleBytes: Data = Data([5, 6, 7]),
        sourceProofBytes: Data = Data(),
        statementHash: String = String(repeating: "56", count: 32),
        backend: String = sccpTronGroth16Bn254ProofBackendV1,
        sourceDomain: UInt32 = sccpDomainSora
    ) throws -> TronSccpProofRequestInput {
        try TronSccpProofRequestInput(
            publicInputs: publicInputs,
            bundleBytes: bundleBytes,
            sourceProofBytes: sourceProofBytes,
            statementHash: statementHash,
            destinationBinding: try sampleTronDestinationBinding(targetDomain: publicInputs.targetDomain),
            backend: backend,
            sourceDomain: sourceDomain
        )
    }

    private static func sampleTronPublicInputs(
        payloadHash: String = String(repeating: "22", count: 32),
        targetDomain: UInt32 = sccpDomainTron
    ) -> TronSccpPublicInputsInput {
        TronSccpPublicInputsInput(
            messageId: String(repeating: "11", count: 32),
            payloadHash: payloadHash,
            targetDomain: targetDomain,
            commitmentRoot: String(repeating: "33", count: 32),
            finalityHeight: 19,
            finalityBlockHash: String(repeating: "44", count: 32)
        )
    }

    private static func sampleEvmProofRequestInput(
        publicInputs: EvmSccpPublicInputsInput = sampleEvmPublicInputs(),
        bundleBytes: Data = Data([5, 6, 7]),
        sourceProofBytes: Data = Data(),
        statementHash: String = String(repeating: "56", count: 32),
        destinationBindingHash: String = String(repeating: "78", count: 32),
        backend: String = sccpEvmGroth16Bn254ProofBackendV1,
        sourceDomain: UInt32 = sccpDomainSora,
        proofArtifactHash: String? = nil,
        provingKeyHash: String? = nil
    ) -> EvmSccpProofRequestInput {
        EvmSccpProofRequestInput(
            publicInputs: publicInputs,
            bundleBytes: bundleBytes,
            sourceProofBytes: sourceProofBytes,
            statementHash: statementHash,
            destinationBindingHash: destinationBindingHash,
            backend: backend,
            sourceDomain: sourceDomain,
            proofArtifactHash: proofArtifactHash,
            provingKeyHash: provingKeyHash
        )
    }

    private static func sampleEvmDestinationBinding(
        targetDomain: UInt32 = sccpDomainEthereum,
        networkId: String = "0x" + String(repeating: "33", count: 32),
        verifierAddress: String = "0x" + String(repeating: "11", count: 20),
        bridgeAddress: String = "0x" + String(repeating: "22", count: 20),
        verifierCodeHash: String = "0x" + String(repeating: "bb", count: 32),
        verifierKeyHash: String = "0x" + String(repeating: "cc", count: 32)
    ) throws -> EvmSccpDestinationBinding {
        try sccpEvmDestinationBinding(
            targetDomain: targetDomain,
            networkId: networkId,
            verifierAddress: verifierAddress,
            bridgeAddress: bridgeAddress,
            verifierCodeHash: verifierCodeHash,
            verifierKeyHash: verifierKeyHash
        )
    }

    private static func sampleProductionEvmProofRequestInput(
        publicInputs: EvmSccpPublicInputsInput = sampleEvmPublicInputs(),
        bundleBytes: Data = Data([5, 6, 7]),
        sourceProofBytes: Data = Data(),
        statementHash: String = String(repeating: "56", count: 32),
        backend: String = sccpEvmGroth16Bn254ProofBackendV1,
        sourceDomain: UInt32 = sccpDomainSora,
        proofArtifactHash: String? = nil,
        provingKeyHash: String? = nil
    ) throws -> EvmSccpProofRequestInput {
        try EvmSccpProofRequestInput(
            publicInputs: publicInputs,
            bundleBytes: bundleBytes,
            sourceProofBytes: sourceProofBytes,
            statementHash: statementHash,
            destinationBinding: try sampleEvmDestinationBinding(targetDomain: publicInputs.targetDomain),
            backend: backend,
            sourceDomain: sourceDomain,
            proofArtifactHash: proofArtifactHash,
            provingKeyHash: provingKeyHash
        )
    }

    private static func sampleEthereumNativeEvmProverBundle(
        destinationBindingHash: String,
        verifierKeyHash: String = "0x" + String(repeating: "cc", count: 32),
        noWasm: Bool = true,
        remoteProverRequired: Bool = false,
        expectedDestinationBindingHash: String? = nil
    ) throws -> EthereumMainnetNativeEvmProverBundle {
        let proofArtifactHash = "0x" + String(repeating: "91", count: 32)
        let provingKeyHash = "0x" + String(repeating: "92", count: 32)
        let artifacts = try sccpEthNativeEvmProverRequiredImplementationsV1
            .sorted { $0.key < $1.key }
            .enumerated()
            .map { index, entry in
                    try EthereumMainnetNativeEvmProverBundleSdkArtifact(
                        sdk: entry.key,
                        implementation: entry.value,
                        proofArtifactHash: proofArtifactHash,
                        provingKeyHash: provingKeyHash,
                        implementationArtifact: "artifacts/eth-mainnet/\(entry.key)-implementation.bin",
                        implementationHash: "0x" + String(
                            repeating: String(format: "%02x", index + 1),
                            count: 32
                    )
                )
            }
        return try EthereumMainnetNativeEvmProverBundle(
            proofArtifact: "artifacts/eth-mainnet/proof-artifact.bin",
            proofArtifactHash: proofArtifactHash,
            provingKey: "artifacts/eth-mainnet/proving-key.bin",
            provingKeyHash: provingKeyHash,
            verifierKey: "artifacts/eth-mainnet/verifier-key.bin",
            verifierKeyHash: verifierKeyHash,
            destinationBindingHash: destinationBindingHash,
            noWasm: noWasm,
            remoteProverRequired: remoteProverRequired,
            nativeSdkArtifacts: artifacts,
            crossSdkFixtureParityArtifact: "artifacts/eth-mainnet/cross-sdk-fixture-parity.json",
            nativeProverSelfTestArtifact: "artifacts/eth-mainnet/native-prover-self-test.json",
            auditHashes: Self.sampleEthereumNativeEvmProverAuditHashes(),
            expectedDestinationBindingHash: expectedDestinationBindingHash
        )
    }

    private static func sampleEthereumNativeEvmProverAuditHashes() -> [String: String] {
        [
            "circuit_security_audit": "0x" + String(repeating: "a1", count: 32),
            "native_implementation_audit": "0x" + String(repeating: "a2", count: 32),
            "reproducible_build_attestation": "0x" + String(repeating: "a3", count: 32),
            "cross_sdk_fixture_parity": "0x" + String(repeating: "a4", count: 32),
            "native_prover_self_test": "0x" + String(repeating: "a5", count: 32),
            "no_wasm_no_remote_scan": "0x" + String(repeating: "a6", count: 32)
        ]
    }

    private static func sha256Hex(_ data: Data) -> String {
        "0x" + Data(SHA256.hash(data: data)).hexEncodedString()
    }

    private static func nativeEvmProverArtifactBytes(_ label: String) -> Data {
        var bytes = [UInt8](repeating: 0, count: 256)
        let labelBytes = Array(label.utf8)
        for index in bytes.indices {
            bytes[index] = UInt8((index * 37 + labelBytes.count * 11) & 0xff)
        }
        for (index, byte) in labelBytes.prefix(bytes.count).enumerated() {
            bytes[index] = byte
        }
        return Data(bytes)
    }

    private static func sampleEthereumNativeEvmProverBundleJson(
        destinationBindingHash: String,
        proofArtifact: String = "artifacts/eth-mainnet/proof-artifact.bin",
        noWasm: Bool = true,
        remoteProverRequired: Bool = false
    ) -> String {
        let proofArtifactHash = "0x" + String(repeating: "91", count: 32)
        let provingKeyHash = "0x" + String(repeating: "92", count: 32)
        let artifacts = sccpEthNativeEvmProverRequiredImplementationsV1
            .sorted { $0.key < $1.key }
            .enumerated()
            .map { index, entry in
                """
                {
                  "sdk": "\(entry.key)",
                  "implementation": "\(entry.value)",
                  "prover_artifact_hash": "\(proofArtifactHash)",
                  "proving_key_hash": "\(provingKeyHash)",
                  "implementation_artifact": "artifacts/eth-mainnet/\(entry.key)-implementation.bin",
                  "implementation_hash": "0x\(String(repeating: String(format: "%02x", index + 1), count: 32))"
                }
                """
            }
            .joined(separator: ",")
        return """
        {
          "schema": "\(sccpNativeEvmProverBundleSchemaV1)",
          "bundle_id": "\(sccpEthNativeEvmProverBundleIdV1)",
          "domain": \(sccpDomainEthereum),
          "chain": "eth",
          "proof_backend": "\(sccpEvmGroth16Bn254ProofBackendV1)",
          "proof_artifact": "\(proofArtifact)",
          "proof_artifact_hash": "\(proofArtifactHash)",
          "proving_key": "artifacts/eth-mainnet/proving-key.bin",
          "proving_key_hash": "\(provingKeyHash)",
          "verifier_key": "artifacts/eth-mainnet/verifier-key.bin",
          "verifier_key_hash": "0x\(String(repeating: "cc", count: 32))",
          "destination_binding_hash": "\(destinationBindingHash)",
          "no_wasm": \(noWasm),
          "remote_prover_required": \(remoteProverRequired),
          "browser_implementation": "pure-typescript",
          "native_sdk_artifacts": [\(artifacts)],
          "cross_sdk_fixture_parity_artifact": "artifacts/eth-mainnet/cross-sdk-fixture-parity.json",
          "native_prover_self_test_artifact": "artifacts/eth-mainnet/native-prover-self-test.json",
          "audit_hashes": {
            "circuit_security_audit": "0x\(String(repeating: "a1", count: 32))",
            "native_implementation_audit": "0x\(String(repeating: "a2", count: 32))",
            "reproducible_build_attestation": "0x\(String(repeating: "a3", count: 32))",
            "cross_sdk_fixture_parity": "0x\(String(repeating: "a4", count: 32))",
            "native_prover_self_test": "0x\(String(repeating: "a5", count: 32))",
            "no_wasm_no_remote_scan": "0x\(String(repeating: "a6", count: 32))"
          }
        }
        """
    }

    private static func sampleEthereumNativeEvmProverParityFixtureJson(
        nativeProverBundle: EthereumMainnetNativeEvmProverBundle,
        swiftCalldataHash: String? = nil
    ) -> String {
        let publicSignalWords = (0..<9)
            .map { index in
                "0x" + String(repeating: String(format: "%02x", index + 0x10), count: 32)
            }
        func sdkResult(calldataHash: String = "0x" + String(repeating: "d3", count: 32)) -> String {
            """
            {
              "receipt_proof_hash": "0x\(String(repeating: "d1", count: 32))",
              "source_proof_hash": "0x\(String(repeating: "d2", count: 32))",
              "destination_binding_hash": "\(nativeProverBundle.destinationBindingHash)",
              "public_signal_words": [\(publicSignalWords.map { "\"\($0)\"" }.joined(separator: ","))],
              "calldata_hash": "\(calldataHash)",
              "torii_submit_payload_hash": "0x\(String(repeating: "d4", count: 32))"
            }
            """
        }
        let sdkResults = sccpEthNativeEvmProverRequiredImplementationsV1
            .keys
            .sorted()
            .map { sdk in
                "\"\(sdk)\": \(sdkResult(calldataHash: sdk == "swift" ? (swiftCalldataHash ?? "0x" + String(repeating: "d3", count: 32)) : "0x" + String(repeating: "d3", count: 32)))"
            }
            .joined(separator: ",")
        return """
        {
          "schema": "\(sccpEthNativeEvmProverParityFixtureSchemaV1)",
          "domain": \(sccpDomainEthereum),
          "chain": "eth",
          "proof_backend": "\(sccpEvmGroth16Bn254ProofBackendV1)",
          "proof_artifact_hash": "\(nativeProverBundle.proofArtifactHash)",
          "proving_key_hash": "\(nativeProverBundle.provingKeyHash)",
          "verifier_key_hash": "\(nativeProverBundle.verifierKeyHash)",
          "destination_binding_hash": "\(nativeProverBundle.destinationBindingHash)",
          "receipt_proof_hash": "0x\(String(repeating: "d1", count: 32))",
          "source_proof_hash": "0x\(String(repeating: "d2", count: 32))",
          "public_signal_words": [\(publicSignalWords.map { "\"\($0)\"" }.joined(separator: ","))],
          "calldata_hash": "0x\(String(repeating: "d3", count: 32))",
          "torii_submit_payload_hash": "0x\(String(repeating: "d4", count: 32))",
          "sdk_results": {
            \(sdkResults)
          }
        }
        """
    }

    private static func sampleEthereumNativeEvmProverSelfTestFixtureJson(
        nativeProverBundle: EthereumMainnetNativeEvmProverBundle,
        swiftProofHash: String? = nil
    ) -> String {
        let publicSignalWords = (0..<9)
            .map { index in
                "0x" + String(repeating: String(format: "%02x", index + 0x20), count: 32)
            }
        let proofHash = "0x" + String(repeating: "e4", count: 32)
        func sdkResult(proofHash sdkProofHash: String) -> String {
            """
            {
              "request_hash": "0x\(String(repeating: "e1", count: 32))",
              "witness_hash": "0x\(String(repeating: "e2", count: 32))",
              "source_proof_hash": "0x\(String(repeating: "e3", count: 32))",
              "proof_hash": "\(sdkProofHash)",
              "public_signal_words": [\(publicSignalWords.map { "\"\($0)\"" }.joined(separator: ","))],
              "calldata_hash": "0x\(String(repeating: "e5", count: 32))",
              "torii_submit_payload_hash": "0x\(String(repeating: "e6", count: 32))"
            }
            """
        }
        let sdkResults = sccpEthNativeEvmProverRequiredImplementationsV1
            .keys
            .sorted()
            .map { sdk in
                "\"\(sdk)\": \(sdkResult(proofHash: sdk == "swift" ? (swiftProofHash ?? proofHash) : proofHash))"
            }
            .joined(separator: ",")
        return """
        {
          "schema": "\(sccpEthNativeEvmProverSelfTestSchemaV1)",
          "domain": \(sccpDomainEthereum),
          "chain": "eth",
          "proof_backend": "\(sccpEvmGroth16Bn254ProofBackendV1)",
          "proof_artifact_hash": "\(nativeProverBundle.proofArtifactHash)",
          "proving_key_hash": "\(nativeProverBundle.provingKeyHash)",
          "verifier_key_hash": "\(nativeProverBundle.verifierKeyHash)",
          "destination_binding_hash": "\(nativeProverBundle.destinationBindingHash)",
          "request_hash": "0x\(String(repeating: "e1", count: 32))",
          "witness_hash": "0x\(String(repeating: "e2", count: 32))",
          "source_proof_hash": "0x\(String(repeating: "e3", count: 32))",
          "proof_hash": "\(proofHash)",
          "public_signal_words": [\(publicSignalWords.map { "\"\($0)\"" }.joined(separator: ","))],
          "calldata_hash": "0x\(String(repeating: "e5", count: 32))",
          "torii_submit_payload_hash": "0x\(String(repeating: "e6", count: 32))",
          "sdk_results": {
            \(sdkResults)
          }
        }
        """
    }

    private static func sampleEvmPublicInputs(
        payloadHash: String = String(repeating: "22", count: 32),
        targetDomain: UInt32 = sccpDomainEthereum,
        finalityHeight: UInt64 = 19
    ) -> EvmSccpPublicInputsInput {
        EvmSccpPublicInputsInput(
            messageId: String(repeating: "11", count: 32),
            payloadHash: payloadHash,
            targetDomain: targetDomain,
            commitmentRoot: String(repeating: "33", count: 32),
            finalityHeight: finalityHeight,
            finalityBlockHash: String(repeating: "44", count: 32)
        )
    }


    private static func sampleSourceVerifierMaterialBytes(domain: UInt32) throws -> Data {
        try canonicalSccpSourceVerifierMaterialBytes(
            sourceDomain: domain,
            sourceTrustAnchorHash: "0x" + String(repeating: "44", count: 32),
            consensusVerifierHash: "0x" + String(repeating: "55", count: 32),
            messageInclusionVerifierHash: "0x" + String(repeating: "66", count: 32),
            finalityPolicyHash: "0x" + String(repeating: "88", count: 32),
            sourceStateVerifierHash: sourceStateVerifierHash(domain: domain),
            bridgeAddress: bridgeAddress(domain: domain),
            sourceBridgeEmitterCodeHash: sourceBridgeCodeHash(domain: domain),
            networkId: networkId(domain: domain),
            ownerAddress: ownerAddress(domain: domain),
            configHash: configHash(domain: domain)
        )
    }

    private static func sampleSourceVerifierMaterialHash(domain: UInt32) throws -> String {
        try sccpSourceVerifierMaterialHash(
            sourceDomain: domain,
            sourceTrustAnchorHash: "0x" + String(repeating: "44", count: 32),
            consensusVerifierHash: "0x" + String(repeating: "55", count: 32),
            messageInclusionVerifierHash: "0x" + String(repeating: "66", count: 32),
            finalityPolicyHash: "0x" + String(repeating: "88", count: 32),
            sourceStateVerifierHash: sourceStateVerifierHash(domain: domain),
            bridgeAddress: bridgeAddress(domain: domain),
            sourceBridgeEmitterCodeHash: sourceBridgeCodeHash(domain: domain),
            networkId: networkId(domain: domain),
            ownerAddress: ownerAddress(domain: domain),
            configHash: configHash(domain: domain)
        )
    }

    private static func sampleSourceAdapterDeploymentHash(
        domain: UInt32,
        adapterVerifierVkHash: String? = nil,
        solanaTowerReplayVerifierHash: String? = nil,
        solanaFullAccountsdbLatticeVerifierHash: String? = nil,
        solanaBankForkChoiceVerifierHash: String? = nil,
        tonMasterchainConfigVerifierHash: String? = nil,
        tonValidatorSetTransitionVerifierHash: String? = nil,
        tonShardAccountsDictionaryVerifierHash: String? = nil
    ) throws -> String {
        try sccpSourceAdapterEngineDeploymentHash(
            sourceDomain: domain,
            sourceTrustAnchorHash: "0x" + String(repeating: "44", count: 32),
            consensusVerifierHash: "0x" + String(repeating: "55", count: 32),
            messageInclusionVerifierHash: "0x" + String(repeating: "66", count: 32),
            finalityPolicyHash: "0x" + String(repeating: "88", count: 32),
            deploymentReceiptHash: "0x" + String(repeating: "aa", count: 32),
            adapterVerifierVkHash: adapterVerifierVkHash,
            sourceStateVerifierHash: sourceStateVerifierHash(domain: domain),
            bridgeAddress: bridgeAddress(domain: domain),
            sourceBridgeEmitterCodeHash: sourceBridgeCodeHash(domain: domain),
            networkId: networkId(domain: domain),
            ownerAddress: ownerAddress(domain: domain),
            configHash: configHash(domain: domain),
            solanaTowerReplayVerifierHash: solanaTowerReplayVerifierHash,
            solanaFullAccountsdbLatticeVerifierHash: solanaFullAccountsdbLatticeVerifierHash,
            solanaBankForkChoiceVerifierHash: solanaBankForkChoiceVerifierHash,
            tonMasterchainConfigVerifierHash: tonMasterchainConfigVerifierHash,
            tonValidatorSetTransitionVerifierHash: tonValidatorSetTransitionVerifierHash,
            tonShardAccountsDictionaryVerifierHash: tonShardAccountsDictionaryVerifierHash
        )
    }

    private static func sampleSolanaFullLightClientGateHash(
        towerReplayHash: String = "0x" + String(repeating: "bb", count: 32),
        fullAccountsdbLatticeHash: String = "0x" + String(repeating: "cc", count: 32),
        bankForkChoiceHash: String = "0x" + String(repeating: "dd", count: 32),
        sourceStateHash: String? = sourceStateVerifierHash(domain: sccpDomainSolana)
    ) throws -> String {
        try sccpSolanaFullLightClientGateHash(
            sourceTrustAnchorHash: "0x" + String(repeating: "44", count: 32),
            consensusVerifierHash: "0x" + String(repeating: "55", count: 32),
            messageInclusionVerifierHash: "0x" + String(repeating: "66", count: 32),
            finalityPolicyHash: "0x" + String(repeating: "88", count: 32),
            deploymentReceiptHash: "0x" + String(repeating: "aa", count: 32),
            solanaTowerReplayVerifierHash: towerReplayHash,
            solanaFullAccountsdbLatticeVerifierHash: fullAccountsdbLatticeHash,
            solanaBankForkChoiceVerifierHash: bankForkChoiceHash,
            sourceStateVerifierHash: sourceStateHash
        )
    }

    private static func sampleTonFullLightClientGateHash(
        masterchainConfigHash: String = "0x" + String(repeating: "bb", count: 32),
        validatorSetTransitionHash: String = "0x" + String(repeating: "cc", count: 32),
        shardAccountsDictionaryHash: String = "0x" + String(repeating: "dd", count: 32)
    ) throws -> String {
        try sccpTonFullLightClientGateHash(
            sourceTrustAnchorHash: "0x" + String(repeating: "44", count: 32),
            consensusVerifierHash: "0x" + String(repeating: "55", count: 32),
            messageInclusionVerifierHash: "0x" + String(repeating: "66", count: 32),
            finalityPolicyHash: "0x" + String(repeating: "88", count: 32),
            deploymentReceiptHash: "0x" + String(repeating: "aa", count: 32),
            tonMasterchainConfigVerifierHash: masterchainConfigHash,
            tonValidatorSetTransitionVerifierHash: validatorSetTransitionHash,
            tonShardAccountsDictionaryVerifierHash: shardAccountsDictionaryHash,
            sourceStateVerifierHash: sourceStateVerifierHash(domain: sccpDomainTon)
        )
    }

    private static func sourceStateVerifierHash(domain: UInt32) -> String? {
        let requiresSourceState = domain == sccpDomainSolana
            || domain == sccpDomainTon
        return requiresSourceState ? "0x" + String(repeating: "77", count: 32) : nil
    }

    private static func bridgeAddress(domain: UInt32) -> String? {
        domain == sccpDomainEthereum || domain == sccpDomainBsc || domain == sccpDomainTron
            ? "0x" + String(repeating: "11", count: 20)
            : nil
    }

    private static func sourceBridgeCodeHash(domain: UInt32) -> String? {
        bridgeAddress(domain: domain) == nil ? nil : "0x" + String(repeating: "77", count: 32)
    }

    private static func networkId(domain: UInt32) -> String? {
        if domain == sccpDomainEthereum {
            return sccpEthereumMainnetNetworkId
        }
        return domain == sccpDomainTron ? "0x" + String(repeating: "33", count: 32) : nil
    }

    private static func ownerAddress(domain: UInt32) -> String? {
        domain == sccpDomainTron ? "0x" + String(repeating: "22", count: 20) : nil
    }

    private static func configHash(domain: UInt32) -> String? {
        if domain == sccpDomainEthereum {
            return "0x871a910500648c68576f7d8fb044de1c494ae24c74f435c87dd451e6ae169c6b"
        }
        return domain == sccpDomainTron
            ? "0xe986dd67bfa2307b4e00cf46bde41a88003a55c5b7fea311fa106614b2252f9d"
            : nil
    }

    private static func rlpString(_ value: Data) -> Data {
        if value.count == 1, value[0] < 0x80 {
            return value
        }
        if value.count < 56 {
            var out = Data([0x80 + UInt8(value.count)])
            out.append(value)
            return out
        }
        let lengthBytes = minimalBeLengthBytes(value.count)
        var out = Data([0xb7 + UInt8(lengthBytes.count)])
        out.append(lengthBytes)
        out.append(value)
        return out
    }

    private static func rlpList(_ fields: [Data]) -> Data {
        let payload = fields.reduce(into: Data()) { out, field in
            out.append(field)
        }
        if payload.count < 56 {
            var out = Data([0xc0 + UInt8(payload.count)])
            out.append(payload)
            return out
        }
        let lengthBytes = minimalBeLengthBytes(payload.count)
        var out = Data([0xf7 + UInt8(lengthBytes.count)])
        out.append(lengthBytes)
        out.append(payload)
        return out
    }

    private static func minimalBeLengthBytes(_ value: Int) -> Data {
        var working = value
        var bytes: [UInt8] = []
        repeat {
            bytes.insert(UInt8(working & 0xff), at: 0)
            working >>= 8
        } while working != 0
        return Data(bytes)
    }

    private static func sampleBscParliaExtra() -> Data {
        var extra = Data(repeating: 0x11, count: 32)
        extra.append(2)
        extra.append(Data(repeating: 0x11, count: 20))
        extra.append(Data(repeating: 0x01, count: 48))
        extra.append(Data(repeating: 0x22, count: 20))
        extra.append(Data(repeating: 0x02, count: 48))
        extra.append(Data(repeating: 0x99, count: 65))
        return extra
    }

    private static func sampleBscParliaHeaderRlp(extraData: Data) -> Data {
        rlpList([
            rlpString(Data(repeating: 0x10, count: 32)),
            rlpString(Data(repeating: 0x11, count: 32)),
            rlpString(Data(repeating: 0x12, count: 20)),
            rlpString(Data(repeating: 0x13, count: 32)),
            rlpString(Data(repeating: 0x14, count: 32)),
            rlpString(Data(repeating: 0x15, count: 32)),
            rlpString(Data(repeating: 0x00, count: 256)),
            rlpString(Data([2])),
            rlpString(Data([1])),
            rlpString(Data([1])),
            rlpString(Data([1])),
            rlpString(Data([1])),
            rlpString(extraData),
            rlpString(Data(repeating: 0x00, count: 32)),
            rlpString(Data(repeating: 0x00, count: 8))
        ])
    }

    private static func sampleEthExecutionHeaderRlp(
        receiptsRoot: Data = Data(repeating: 0x15, count: 32)
    ) -> Data {
        rlpList([
            rlpString(Data(repeating: 0x10, count: 32)),
            rlpString(Data(repeating: 0x11, count: 32)),
            rlpString(Data(repeating: 0x12, count: 20)),
            rlpString(Data(repeating: 0x13, count: 32)),
            rlpString(Data(repeating: 0x14, count: 32)),
            rlpString(receiptsRoot),
            rlpString(Data(repeating: 0x00, count: 256)),
            rlpString(Data()),
            rlpString(Data([0x2a])),
            rlpString(Data([0x01, 0xc9, 0xc3, 0x80])),
            rlpString(Data([0x52, 0x08])),
            rlpString(Data([0x65, 0x53, 0xf1, 0x00])),
            rlpString(Data("iroha-sccp-test".utf8)),
            rlpString(Data(repeating: 0x16, count: 32)),
            rlpString(Data(repeating: 0x00, count: 8)),
            rlpString(Data([0x3b, 0x9a, 0xca, 0x00])),
            rlpString(Data(repeating: 0x17, count: 32)),
            rlpString(Data()),
            rlpString(Data()),
            rlpString(Data(repeating: 0x18, count: 32))
        ])
    }
}
