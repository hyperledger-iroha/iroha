import Foundation
import XCTest
@testable import IrohaSwift

final class NexusAppClientTests: XCTestCase {
    private static let assetDefinitionID = "7EAD8EFYUx1aVKZPUU1fyKvr8dF1"
    private static let publicKey = Data(hexString: "d04ab232742bb4ab3a1368bd4615e4e6d0224ab71a016baf8520a332c9778737")!
    private static let walletSignature = Data(hexString: "c82d2ee732a9251153eff6f510a0d12b292cb51a5d961a7eddb84f6ee944e34eaca60ca2f1ccfe7a53fd6813fc9a6db9e35cb276b2411b7d583d45fdc6caee05")!
    private static let accountID = "sorauﾛ1PｸCｶrﾑhyﾜｴﾄhｳﾔSqP2GFGﾗヱﾐｹﾇﾏzﾍｵﾐMﾇﾖﾄksJヱRRJXVB"
    private static let destinationAccountID = "sorauﾛ1Prﾇuﾉﾉ4ﾒdﾛﾑｲﾄn5tﾆﾒrsR9ﾋ2Gｷ7gWeFzyﾁﾋﾁAHﾌTJQQ4L"

    func testTransferWithWalletBuildsSignsSubmitsAndWaits() async throws {
        let connect = FakeConnect()
        let torii = FakeToriiSubmitter()
        let client = NexusAppClient(
            config: NexusAppConfig(chainId: "test-chain",
                                   appId: "sample-app",
                                   signingPublicKey: Self.publicKey),
            connectTransport: connect,
            toriiSubmitter: torii
        )

        let session = try await client.startConnect(
            options: NexusConnectOptions(walletURIBase: URL(string: "sora://wallet/connect")!)
        )
        let approved = try await client.awaitApproval(session: session)
        let approvedSession = try XCTUnwrap(approved.session)
        let receipt = try await client.transferWithWallet(
            session: approvedSession,
            input: sampleInput(),
            options: NexusFinalizeOptions(waitForFinalStatus: true)
        )

        XCTAssertEqual(approved.accountID, Self.accountID)
        XCTAssertEqual(receipt.finalStatus, "Committed")
        XCTAssertEqual(receipt.transactionHashHex, torii.submittedHash)
        XCTAssertEqual(receipt.transactionHashHex, receipt.signedTransaction.hashHex)
        XCTAssertEqual(receipt.signedTransaction.payload, connect.lastSignable?.payloadBytes)
        XCTAssertEqual(receipt.signedTransaction.signedTransaction.isEmpty, false)
    }

    func testBuildTransferDraftFailsClosedWithoutSigningPublicKey() throws {
        let client = NexusAppClient(
            config: NexusAppConfig(chainId: "test-chain", authority: Self.accountID)
        )

        let error = try expectNexusError {
            _ = try client.buildTransferDraft(input: sampleInput())
        }

        XCTAssertEqual(error.code, "missing_signing_public_key")
    }

    func testBuildTransferDraftUsesSharedFixturePayload() throws {
        let fixture = try Self.loadNexusFixture()
        let expected = try XCTUnwrap(fixture["expected"] as? [String: Any])
        let expectedPayloadHash = try XCTUnwrap(expected["payload_hash_hex"] as? String)
        let expectedPayloadBytes = try Self.hexData(
            try XCTUnwrap(expected["payload_bytes_hex"] as? String)
        )
        let client = NexusAppClient(
            config: NexusAppConfig(chainId: "test-chain",
                                   authority: Self.accountID,
                                   signingPublicKey: Self.publicKey)
        )

        let draft = try client.buildTransferDraft(input: sampleInput())

        XCTAssertEqual(draft.signable.payloadHashHex, expectedPayloadHash)
        XCTAssertEqual(draft.signable.payloadBytes, expectedPayloadBytes)
    }

    func testFinalizeAndSubmitUsesSharedFixtureSignedTransactionHash() async throws {
        let fixture = try Self.loadNexusFixture()
        let expected = try XCTUnwrap(fixture["expected"] as? [String: Any])
        let expectedTransactionHash = try XCTUnwrap(expected["signed_transaction_hash_hex"] as? String)
        let walletSignature = try Self.hexData(
            try XCTUnwrap(expected["wallet_signature_hex"] as? String)
        )
        let torii = FakeToriiSubmitter()
        let client = NexusAppClient(
            config: NexusAppConfig(chainId: "test-chain",
                                   authority: Self.accountID,
                                   signingPublicKey: Self.publicKey),
            toriiSubmitter: torii
        )
        let draft = try client.buildTransferDraft(input: sampleInput())

        let receipt = try await client.finalizeAndSubmit(
            signable: draft.signable,
            signature: NexusWalletSignature(signature: walletSignature),
            options: NexusFinalizeOptions(waitForFinalStatus: false)
        )

        XCTAssertEqual(receipt.transactionHashHex, expectedTransactionHash)
        XCTAssertEqual(receipt.signedTransaction.hashHex, expectedTransactionHash)
        XCTAssertEqual(torii.submittedHash, expectedTransactionHash)
    }

    func testFinalizeAndSubmitRejectsUnsupportedSignatureAlgorithm() async throws {
        let client = NexusAppClient(
            config: NexusAppConfig(chainId: "test-chain",
                                   authority: Self.accountID,
                                   signingPublicKey: Self.publicKey),
            toriiSubmitter: FakeToriiSubmitter()
        )
        let draft = try client.buildTransferDraft(input: sampleInput())

        let error = await expectNexusErrorAsync {
            _ = try await client.finalizeAndSubmit(
                signable: draft.signable,
                signature: NexusWalletSignature(signature: Data(repeating: 0x07, count: 64),
                                                 algorithm: "secp256k1")
            )
        }

        XCTAssertEqual(error.code, "unsupported_signature_algorithm")
    }

    func testAwaitApprovalRejectsMissingAccountAndSigningKey() async throws {
        let missingAccount = NexusAppClient(
            config: NexusAppConfig(chainId: "test-chain"),
            connectTransport: ApprovalConnect(
                approval: NexusApprovedAccount(accountID: "")
            )
        )
        let accountError = await expectNexusErrorAsync {
            _ = try await missingAccount.awaitApproval(
                session: NexusConnectSession(sessionID: "session-1",
                                             walletLaunchURI: URL(string: "sora://wallet/connect")!)
            )
        }
        XCTAssertEqual(accountError.code, "approval_missing_account")

        let missingKey = NexusAppClient(
            config: NexusAppConfig(chainId: "test-chain"),
            connectTransport: ApprovalConnect(
                approval: NexusApprovedAccount(accountID: Self.accountID)
            )
        )
        let keyError = await expectNexusErrorAsync {
            _ = try await missingKey.awaitApproval(
                session: NexusConnectSession(sessionID: "session-1",
                                             walletLaunchURI: URL(string: "sora://wallet/connect")!)
            )
        }
        XCTAssertEqual(keyError.code, "missing_signing_public_key")

        let invalidKey = NexusAppClient(
            config: NexusAppConfig(chainId: "test-chain"),
            connectTransport: ApprovalConnect(
                approval: NexusApprovedAccount(accountID: Self.accountID,
                                               signingPublicKey: Data(repeating: 0x01, count: 31))
            )
        )
        let invalidKeyError = await expectNexusErrorAsync {
            _ = try await invalidKey.awaitApproval(
                session: NexusConnectSession(sessionID: "session-1",
                                             walletLaunchURI: URL(string: "sora://wallet/connect")!)
            )
        }
        XCTAssertEqual(invalidKeyError.code, "invalid_signing_public_key")
    }

    func testTransferWithWalletRejectsAuthorityMismatchBeforeSigning() async throws {
        let connect = FakeConnect()
        let client = NexusAppClient(
            config: NexusAppConfig(chainId: "test-chain",
                                   signingPublicKey: Self.publicKey),
            connectTransport: connect,
            toriiSubmitter: FakeToriiSubmitter()
        )
        let session = NexusConnectSession(sessionID: "session-1",
                                          walletLaunchURI: URL(string: "sora://wallet/connect")!,
                                          approvedAccount: Self.accountID,
                                          signingPublicKey: Self.publicKey)

        let error = await expectNexusErrorAsync {
            _ = try await client.transferWithWallet(
                session: session,
                input: NexusTransferInput(sourceAssetID: "\(Self.assetDefinitionID)#\(Self.destinationAccountID)",
                                          quantity: "12.34",
                                          destinationAccountID: Self.destinationAccountID,
                                          authority: Self.destinationAccountID,
                                          signingPublicKey: Self.publicKey)
            )
        }

        XCTAssertEqual(error.code, "approval_account_mismatch")
        XCTAssertNil(connect.lastSignable)
    }

    func testFinalizeAndSubmitRejectsInvalidSignatureLength() async throws {
        let client = NexusAppClient(
            config: NexusAppConfig(chainId: "test-chain",
                                   authority: Self.accountID,
                                   signingPublicKey: Self.publicKey),
            toriiSubmitter: FakeToriiSubmitter()
        )
        let draft = try client.buildTransferDraft(input: sampleInput())

        let error = await expectNexusErrorAsync {
            _ = try await client.finalizeAndSubmit(
                signable: draft.signable,
                signature: NexusWalletSignature(signature: Data(repeating: 0x07, count: 63))
            )
        }

        XCTAssertEqual(error.code, "invalid_signature")
    }

    func testFinalizeAndSubmitRejectsInvalidSignatureBytes() async throws {
        let client = NexusAppClient(
            config: NexusAppConfig(chainId: "test-chain",
                                   authority: Self.accountID,
                                   signingPublicKey: Self.publicKey),
            toriiSubmitter: FakeToriiSubmitter()
        )
        let draft = try client.buildTransferDraft(input: sampleInput())

        let error = await expectNexusErrorAsync {
            _ = try await client.finalizeAndSubmit(
                signable: draft.signable,
                signature: NexusWalletSignature(signature: Data(repeating: 0x07, count: 64))
            )
        }

        XCTAssertEqual(error.code, "invalid_signature")
    }

    func testFinalizeAndSubmitRejectsHashMismatchAndMapsSubmitStatusFailures() async throws {
        let draftClient = NexusAppClient(
            config: NexusAppConfig(chainId: "test-chain",
                                   authority: Self.accountID,
                                   signingPublicKey: Self.publicKey),
            toriiSubmitter: FakeToriiSubmitter()
        )
        let draft = try draftClient.buildTransferDraft(input: sampleInput())
        let signature = NexusWalletSignature(signature: Self.walletSignature)

        let mismatchClient = NexusAppClient(
            config: NexusAppConfig(chainId: "test-chain"),
            toriiSubmitter: FakeToriiSubmitter(responseHash: String(repeating: "f", count: 64))
        )
        let mismatchError = await expectNexusErrorAsync {
            _ = try await mismatchClient.finalizeAndSubmit(signable: draft.signable,
                                                           signature: signature)
        }
        XCTAssertEqual(mismatchError.code, "transaction_hash_mismatch")

        let submitFailureClient = NexusAppClient(
            config: NexusAppConfig(chainId: "test-chain"),
            toriiSubmitter: FakeToriiSubmitter(submitError: FakeToriiError.down)
        )
        let submitError = await expectNexusErrorAsync {
            _ = try await submitFailureClient.finalizeAndSubmit(signable: draft.signable,
                                                                signature: signature)
        }
        XCTAssertEqual(submitError.code, "submit_failed")

        let statusFailureClient = NexusAppClient(
            config: NexusAppConfig(chainId: "test-chain"),
            toriiSubmitter: FakeToriiSubmitter(statusError: FakeToriiError.timeout)
        )
        let statusError = await expectNexusErrorAsync {
            _ = try await statusFailureClient.finalizeAndSubmit(signable: draft.signable,
                                                                signature: signature)
        }
        XCTAssertEqual(statusError.code, "status_wait_failed")
    }

    private static func sampleInput() -> NexusTransferInput {
        NexusTransferInput(
            sourceAssetID: "\(assetDefinitionID)#\(accountID)",
            quantity: "12.34",
            destinationAccountID: destinationAccountID,
            creationTimeMs: 1_700_000_000_000,
            ttlMs: 30_000,
            nonce: 7,
            metadata: ["purpose": "nexus-app-fixture"]
        )
    }

    private func sampleInput() -> NexusTransferInput {
        Self.sampleInput()
    }

    private static func loadNexusFixture() throws -> [String: Any] {
        var url = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
        while true {
            let candidate = url.appendingPathComponent("fixtures/sdk/nexus_connect_transfer_v1.json")
            if FileManager.default.fileExists(atPath: candidate.path) {
                let data = try Data(contentsOf: candidate)
                let json = try JSONSerialization.jsonObject(with: data)
                guard let object = json as? [String: Any] else {
                    throw NexusAppError(code: "invalid_fixture", message: "fixture root must be an object")
                }
                return object
            }
            let parent = url.deletingLastPathComponent()
            if parent.path == url.path { break }
            url = parent
        }
        throw NexusAppError(code: "missing_fixture", message: "Nexus fixture was not found")
    }

    private static func hexData(_ hex: String) throws -> Data {
        guard hex.count % 2 == 0 else {
            throw NexusAppError(code: "invalid_hex", message: "hex length must be even")
        }
        var data = Data()
        var index = hex.startIndex
        while index < hex.endIndex {
            let next = hex.index(index, offsetBy: 2)
            guard let byte = UInt8(hex[index..<next], radix: 16) else {
                throw NexusAppError(code: "invalid_hex", message: "invalid hex byte")
            }
            data.append(byte)
            index = next
        }
        return data
    }

    private func expectNexusError(_ block: () throws -> Void) throws -> NexusAppError {
        do {
            try block()
        } catch let error as NexusAppError {
            return error
        }
        XCTFail("Expected NexusAppError")
        return NexusAppError(code: "missing_test_error", message: "Expected NexusAppError")
    }

    private func expectNexusErrorAsync(_ block: () async throws -> Void) async -> NexusAppError {
        do {
            try await block()
        } catch let error as NexusAppError {
            return error
        } catch {
            XCTFail("Expected NexusAppError, got \(error)")
            return NexusAppError(code: "wrong_test_error", message: "Expected NexusAppError")
        }
        XCTFail("Expected NexusAppError")
        return NexusAppError(code: "missing_test_error", message: "Expected NexusAppError")
    }

    private final class FakeConnect: NexusConnectTransport {
        private(set) var lastSignable: NexusSignableTransaction?

        func startConnect(options: NexusConnectOptions,
                          config: NexusAppConfig) async throws -> NexusConnectSession {
            let base = options.walletURIBase ?? URL(string: "sora://wallet/connect")!
            let sessionID = options.sessionID ?? "session-1"
            return NexusConnectSession(sessionID: sessionID,
                                       walletLaunchURI: URL(string: "\(base.absoluteString)?session=\(sessionID)")!,
                                       appId: config.appId,
                                       relayURL: config.relayURL,
                                       node: options.node ?? config.node)
        }

        func awaitApproval(session: NexusConnectSession,
                           config: NexusAppConfig) async throws -> NexusApprovedAccount {
            NexusApprovedAccount(accountID: NexusAppClientTests.accountID,
                                 signingPublicKey: NexusAppClientTests.publicKey)
        }

        func requestSignature(session: NexusConnectSession,
                              signable: NexusSignableTransaction,
                              config: NexusAppConfig) async throws -> NexusWalletSignature {
            lastSignable = signable
            XCTAssertEqual(signable.signatureAlgorithm, NexusSignatureAlgorithmEd25519)
            return NexusWalletSignature(signature: NexusAppClientTests.walletSignature)
        }
    }

    private final class ApprovalConnect: NexusConnectTransport {
        let approval: NexusApprovedAccount

        init(approval: NexusApprovedAccount) {
            self.approval = approval
        }

        func startConnect(options: NexusConnectOptions,
                          config: NexusAppConfig) async throws -> NexusConnectSession {
            NexusConnectSession(sessionID: "session-1",
                                walletLaunchURI: URL(string: "sora://wallet/connect")!)
        }

        func awaitApproval(session: NexusConnectSession,
                           config: NexusAppConfig) async throws -> NexusApprovedAccount {
            approval
        }

        func requestSignature(session: NexusConnectSession,
                              signable: NexusSignableTransaction,
                              config: NexusAppConfig) async throws -> NexusWalletSignature {
            throw NexusAppError(code: "unexpected_signature_request",
                                message: "signature request should not be called")
        }
    }

    private enum FakeToriiError: Error {
        case down
        case timeout
    }

    private final class FakeToriiSubmitter: NexusToriiSubmitting {
        private(set) var submittedHash: String?
        private let responseHash: String?
        private let submitError: Error?
        private let statusError: Error?

        init(responseHash: String? = nil,
             submitError: Error? = nil,
             statusError: Error? = nil) {
            self.responseHash = responseHash
            self.submitError = submitError
            self.statusError = statusError
        }

        func submitNexusTransaction(_ envelope: SignedTransactionEnvelope) async throws -> ToriiSubmitTransactionResponse? {
            if let submitError {
                throw submitError
            }
            submittedHash = envelope.hashHex
            return ToriiSubmitTransactionResponse(
                payload: ToriiSubmitTransactionResponse.Payload(
                    txHash: responseHash ?? envelope.hashHex,
                    submittedAtMs: 1_700_000_000_500,
                    submittedAtHeight: 7,
                    signer: NexusAppClientTests.accountID
                ),
                signature: "receipt-signature"
            )
        }

        func waitForNexusTransactionStatus(hashHex: String,
                                           options: PipelineStatusPollOptions) async throws -> String {
            if let statusError {
                throw statusError
            }
            XCTAssertEqual(hashHex, submittedHash)
            return "Committed"
        }
    }
}
