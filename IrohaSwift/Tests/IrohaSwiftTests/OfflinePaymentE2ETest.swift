import CryptoKit
import Foundation
import XCTest
@testable import IrohaSwift

// MARK: - E2E test for offline cash settlement
//
// Mirrors the Android OfflinePaymentE2ETest:
//   1. Alice: setup  → load 5  → send 4 to Bob  → sync (outgoing)
//   2. Bob:   setup  → sync incoming receipt       → balance == 4
//
// Requires a running local Iroha node at IROHA_NODE_URL (default http://127.0.0.1:8080),
// a local `iroha` CLI, and a compatible client config. Skips automatically and
// quickly when the environment is not provisioned.

@available(iOS 15.0, macOS 12.0, *)
final class OfflinePaymentE2ETest: XCTestCase {

    // MARK: - Configuration

    private struct MintContext {
        let cliURL: URL
        let configURL: URL
    }

    private static let repositoryRoot: URL = {
        URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent() // OfflinePaymentE2ETest.swift
            .deletingLastPathComponent() // IrohaSwiftTests
            .deletingLastPathComponent() // Tests
            .deletingLastPathComponent() // IrohaSwift
    }()

    private static let nodeURL: URL = {
        let env = ProcessInfo.processInfo.environment["IROHA_NODE_URL"]
            ?? "http://127.0.0.1:8080"
        return URL(string: env)!
    }()

    private static let assetDefinitionId = "7EAD8EFYUx1aVKZPUU1fyKvr8dF1"
    private static let probeTimeout: TimeInterval = 2
    private static let cliEnvKey = "IROHA_CLI_PATH"
    private static let clientConfigEnvKey = "IROHA_CLIENT_CONFIG"

    // MARK: - Per-participant state

    private final class DeviceIdentity {
        let privateKey: Curve25519.Signing.PrivateKey
        let publicKeyBase64: String
        let deviceId: String
        let attestationKeyId: String
        private(set) var counter: UInt64 = 0

        init() {
            let pk = Curve25519.Signing.PrivateKey()
            self.privateKey = pk
            self.publicKeyBase64 = pk.publicKey.rawRepresentation.base64EncodedString()
            self.deviceId = UUID().uuidString
            self.attestationKeyId = SHA256.hash(data: pk.publicKey.rawRepresentation)
                .map { String(format: "%02x", $0) }.joined()
        }

        func sign(_ data: Data) throws -> Data {
            try privateKey.signature(for: data)
        }

        func nextCounter() -> UInt64 {
            counter += 1
            return counter
        }
    }

    private var client: ToriiClient!
    private var mintContext: MintContext!

    // MARK: - Lifecycle

    override func setUp() async throws {
        try await super.setUp()
        client = ToriiClient(baseURL: Self.nodeURL)
        mintContext = try Self.resolveMintContext()

        // Skip if node is unreachable.
        var probe = URLRequest(url: Self.nodeURL.appendingPathComponent("status"))
        probe.timeoutInterval = Self.probeTimeout
        let sessionConfig = URLSessionConfiguration.ephemeral
        sessionConfig.timeoutIntervalForRequest = Self.probeTimeout
        sessionConfig.timeoutIntervalForResource = Self.probeTimeout
        let session = URLSession(configuration: sessionConfig)
        do {
            let (_, response) = try await session.data(for: probe)
            guard let http = response as? HTTPURLResponse, (200..<300).contains(http.statusCode) else {
                throw XCTSkip("Iroha node returned non-200 at \(Self.nodeURL)")
            }
        } catch is XCTSkip {
            throw XCTSkip("Iroha node unreachable at \(Self.nodeURL)")
        } catch {
            throw XCTSkip("Iroha node unreachable at \(Self.nodeURL): \(error)")
        }
    }

    // MARK: - Test

    func testOfflinePayment_aliceSendsToBob_balancesUpdate() async throws {
        let alice = DeviceIdentity()
        let bob = DeviceIdentity()

        // Use existing pre-minted accounts. In a real E2E infra these would be created by the test.
        // For now, create unique account IDs from public keys.
        let aliceAccountId = try makeAccountId(publicKey: alice.privateKey.publicKey.rawRepresentation)
        let bobAccountId = try makeAccountId(publicKey: bob.privateKey.publicKey.rawRepresentation)

        // === Register and fund both accounts ===
        try await registerAndFundAccount(accountId: aliceAccountId)
        try await registerAndFundAccount(accountId: bobAccountId)

        // === Alice: setup ===
        let aliceSetup = try await callSetup(accountId: aliceAccountId, identity: alice)
        print("Alice setup: lineageId=\(aliceSetup.lineageState.lineageId), balance=\(aliceSetup.lineageState.balance)")

        // === Verify anchor hash matches serverStateHash ===
        let computedAnchor = try computeAnchorHash(state: aliceSetup.lineageState)
        XCTAssertEqual(
            computedAnchor,
            aliceSetup.lineageState.serverStateHash,
            "computeAnchorHash must match serverStateHash from server"
        )
        print("Anchor hash verified: \(computedAnchor)")

        // === Alice: load 2 ===
        let aliceLoad1 = try await callLoad(
            accountId: aliceAccountId,
            lineageId: aliceSetup.lineageState.lineageId,
            amount: "2",
            identity: alice
        )
        print("Alice load1: balance=\(aliceLoad1.lineageState.balance)")

        // === Alice: load 3 ===
        let aliceLoad2 = try await callLoad(
            accountId: aliceAccountId,
            lineageId: aliceSetup.lineageState.lineageId,
            amount: "3",
            identity: alice
        )
        print("Alice load2: balance=\(aliceLoad2.lineageState.balance)")
        XCTAssertEqual(aliceLoad2.lineageState.balance, "5", "Alice should have balance 5 after loading 2 + 3")

        // === Bob: setup ===
        let bobSetup = try await callSetup(accountId: bobAccountId, identity: bob)
        print("Bob setup: lineageId=\(bobSetup.lineageState.lineageId), balance=\(bobSetup.lineageState.balance)")

        // === Bob: load 50 (simulate top-up like real app) ===
        let bobLoad = try await callLoad(
            accountId: bobAccountId,
            lineageId: bobSetup.lineageState.lineageId,
            amount: "50",
            identity: bob
        )
        print("Bob load: balance=\(bobLoad.lineageState.balance)")
        XCTAssertEqual(bobLoad.lineageState.balance, "50", "Bob should have balance 50 after loading")

        // === Build outgoing receipt: Alice sends 4 to Bob ===
        let transferId = UUID().uuidString.uppercased()
        let transferAmount = "4"
        let aliceState = aliceLoad2.lineageState
        let bobState = bobLoad.lineageState

        let outgoingReceipt = try buildOutgoingReceipt(
            transferId: transferId,
            amount: transferAmount,
            senderState: aliceState,
            receiverState: bobState,
            senderIdentity: alice
        )
        print("Outgoing receipt built: transferId=\(transferId)")

        // === Alice: sync outgoing receipt ===
        let aliceSync = try await callSync(
            accountId: aliceAccountId,
            lineageId: aliceState.lineageId,
            identity: alice,
            receipts: [outgoingReceipt]
        )
        print("Alice sync: balance=\(aliceSync.lineageState.balance)")
        XCTAssertEqual(aliceSync.lineageState.balance, "1", "Alice should have balance 1 after sending 4")

        // === Build incoming receipt: Bob receives 4 from Alice ===
        let incomingReceipt = try buildIncomingReceipt(
            transferId: transferId,
            amount: transferAmount,
            senderState: aliceState,
            receiverState: bobState,
            outgoingReceipt: outgoingReceipt,
            receiverIdentity: bob
        )
        print("Incoming receipt built")

        // === Bob: sync incoming receipt ===
        let bobSync = try await callSync(
            accountId: bobAccountId,
            lineageId: bobState.lineageId,
            identity: bob,
            receipts: [incomingReceipt]
        )
        print("Bob sync: balance=\(bobSync.lineageState.balance)")
        XCTAssertEqual(bobSync.lineageState.balance, "54", "Bob should have balance 54 after loading 50 + receiving 4")
    }

    // MARK: - Receipt building

    private func buildOutgoingReceipt(
        transferId: String,
        amount: String,
        senderState: ToriiOfflineCashState,
        receiverState: ToriiOfflineCashState,
        senderIdentity: DeviceIdentity
    ) throws -> ToriiOfflineTransferReceipt {
        let createdAtMs = UInt64(Date().timeIntervalSince1970 * 1000)
        let postBalance = subtractAmounts(senderState.balance, amount)
        let postLockedBalance = "0"
        let localRevision = senderState.pendingLocalRevision + 1

        let postStateHash = try computeLocalStateHash(
            lineageId: senderState.lineageId,
            previousStateHash: senderState.serverStateHash,
            transferId: transferId,
            direction: "outgoing",
            counterpartyLineageId: receiverState.lineageId,
            amount: amount,
            localRevision: localRevision,
            postBalance: postBalance,
            postLockedBalance: postLockedBalance
        )

        let challengeHashHex = try computeChallengeHash(
            accountId: senderState.accountId,
            lineageId: senderState.lineageId,
            operation: "send",
            innerPayload: sortedJSON([
                "amount": amount,
                "lineage_id": senderState.lineageId,
                "receiver_lineage_id": receiverState.lineageId,
                "transfer_id": transferId,
            ])
        )

        let senderCounter = senderIdentity.nextCounter()
        let assertionBase64 = stubAssertionBase64(counter: senderCounter)

        let unsignedReceipt = ToriiOfflineTransferReceipt(
            transferId: transferId,
            direction: .outgoing,
            lineageId: senderState.lineageId,
            accountId: senderState.accountId,
            deviceId: senderIdentity.deviceId,
            offlinePublicKey: senderIdentity.publicKeyBase64,
            preBalance: senderState.balance,
            postBalance: postBalance,
            preLockedBalance: senderState.lockedBalance,
            postLockedBalance: postLockedBalance,
            preStateHash: senderState.serverStateHash,
            postStateHash: postStateHash,
            localRevision: localRevision,
            counterpartyLineageId: receiverState.lineageId,
            counterpartyAccountId: receiverState.accountId,
            counterpartyDeviceId: receiverState.deviceId,
            counterpartyOfflinePublicKey: receiverState.offlinePublicKey,
            amount: amount,
            authorization: senderState.authorization,
            deviceProof: ToriiOfflineDeviceProof(
                platform: "ios",
                attestationKeyId: senderIdentity.attestationKeyId,
                challengeHashHex: challengeHashHex,
                assertionBase64: assertionBase64,
                counter: senderCounter
            ),
            sourcePayload: nil,
            senderSignatureBase64: "",
            createdAtMs: createdAtMs
        )

        let unsignedPayload = try buildCashUnsignedPayloadBytes(unsignedReceipt)
        let payloadHashHex = sha256Hex(unsignedPayload)
        print("Outgoing unsigned payload hash: \(payloadHashHex) size: \(unsignedPayload.count)")

        let signature = try senderIdentity.sign(unsignedPayload)
        return ToriiOfflineTransferReceipt(
            transferId: unsignedReceipt.transferId,
            direction: unsignedReceipt.direction,
            lineageId: unsignedReceipt.lineageId,
            accountId: unsignedReceipt.accountId,
            deviceId: unsignedReceipt.deviceId,
            offlinePublicKey: unsignedReceipt.offlinePublicKey,
            preBalance: unsignedReceipt.preBalance,
            postBalance: unsignedReceipt.postBalance,
            preLockedBalance: unsignedReceipt.preLockedBalance,
            postLockedBalance: unsignedReceipt.postLockedBalance,
            preStateHash: unsignedReceipt.preStateHash,
            postStateHash: unsignedReceipt.postStateHash,
            localRevision: unsignedReceipt.localRevision,
            counterpartyLineageId: unsignedReceipt.counterpartyLineageId,
            counterpartyAccountId: unsignedReceipt.counterpartyAccountId,
            counterpartyDeviceId: unsignedReceipt.counterpartyDeviceId,
            counterpartyOfflinePublicKey: unsignedReceipt.counterpartyOfflinePublicKey,
            amount: unsignedReceipt.amount,
            authorization: unsignedReceipt.authorization,
            deviceProof: unsignedReceipt.deviceProof,
            sourcePayload: unsignedReceipt.sourcePayload,
            senderSignatureBase64: signature.base64EncodedString(),
            createdAtMs: unsignedReceipt.createdAtMs
        )
    }

    private func buildIncomingReceipt(
        transferId: String,
        amount: String,
        senderState: ToriiOfflineCashState,
        receiverState: ToriiOfflineCashState,
        outgoingReceipt: ToriiOfflineTransferReceipt,
        receiverIdentity: DeviceIdentity
    ) throws -> ToriiOfflineTransferReceipt {
        let createdAtMs = UInt64(Date().timeIntervalSince1970 * 1000)
        let postBalance = addAmounts(receiverState.balance, amount)
        let postLockedBalance = "0"
        let localRevision = receiverState.pendingLocalRevision + 1

        let postStateHash = try computeLocalStateHash(
            lineageId: receiverState.lineageId,
            previousStateHash: receiverState.serverStateHash,
            transferId: transferId,
            direction: "incoming",
            counterpartyLineageId: senderState.lineageId,
            amount: amount,
            localRevision: localRevision,
            postBalance: postBalance,
            postLockedBalance: postLockedBalance
        )

        let challengeHashHex = try computeChallengeHash(
            accountId: receiverState.accountId,
            lineageId: receiverState.lineageId,
            operation: "receive",
            innerPayload: sortedJSON([
                "amount": amount,
                "lineage_id": receiverState.lineageId,
                "sender_lineage_id": senderState.lineageId,
                "transfer_id": transferId,
            ])
        )

        let receiverCounter = receiverIdentity.nextCounter()
        let receiverAssertionBase64 = stubAssertionBase64(counter: receiverCounter)

        // Build source payload (outgoing transfer payload for the receiver)
        let sourcePayload = try buildSourcePayload(senderState: senderState, outgoingReceipt: outgoingReceipt)

        let unsignedReceipt = ToriiOfflineTransferReceipt(
            transferId: transferId,
            direction: .incoming,
            lineageId: receiverState.lineageId,
            accountId: receiverState.accountId,
            deviceId: receiverIdentity.deviceId,
            offlinePublicKey: receiverIdentity.publicKeyBase64,
            preBalance: receiverState.balance,
            postBalance: postBalance,
            preLockedBalance: receiverState.lockedBalance,
            postLockedBalance: postLockedBalance,
            preStateHash: receiverState.serverStateHash,
            postStateHash: postStateHash,
            localRevision: localRevision,
            counterpartyLineageId: senderState.lineageId,
            counterpartyAccountId: senderState.accountId,
            counterpartyDeviceId: senderState.deviceId,
            counterpartyOfflinePublicKey: senderState.offlinePublicKey,
            amount: amount,
            authorization: receiverState.authorization,
            deviceProof: ToriiOfflineDeviceProof(
                platform: "ios",
                attestationKeyId: receiverIdentity.attestationKeyId,
                challengeHashHex: challengeHashHex,
                assertionBase64: receiverAssertionBase64,
                counter: receiverCounter
            ),
            sourcePayload: sourcePayload,
            senderSignatureBase64: "",
            createdAtMs: createdAtMs
        )

        let unsignedPayload = try buildCashUnsignedPayloadBytes(unsignedReceipt)
        let payloadHashHex = sha256Hex(unsignedPayload)
        print("Incoming unsigned payload hash: \(payloadHashHex) size: \(unsignedPayload.count)")

        let signature = try receiverIdentity.sign(unsignedPayload)
        return ToriiOfflineTransferReceipt(
            transferId: unsignedReceipt.transferId,
            direction: unsignedReceipt.direction,
            lineageId: unsignedReceipt.lineageId,
            accountId: unsignedReceipt.accountId,
            deviceId: unsignedReceipt.deviceId,
            offlinePublicKey: unsignedReceipt.offlinePublicKey,
            preBalance: unsignedReceipt.preBalance,
            postBalance: unsignedReceipt.postBalance,
            preLockedBalance: unsignedReceipt.preLockedBalance,
            postLockedBalance: unsignedReceipt.postLockedBalance,
            preStateHash: unsignedReceipt.preStateHash,
            postStateHash: unsignedReceipt.postStateHash,
            localRevision: unsignedReceipt.localRevision,
            counterpartyLineageId: unsignedReceipt.counterpartyLineageId,
            counterpartyAccountId: unsignedReceipt.counterpartyAccountId,
            counterpartyDeviceId: unsignedReceipt.counterpartyDeviceId,
            counterpartyOfflinePublicKey: unsignedReceipt.counterpartyOfflinePublicKey,
            amount: unsignedReceipt.amount,
            authorization: unsignedReceipt.authorization,
            deviceProof: unsignedReceipt.deviceProof,
            sourcePayload: unsignedReceipt.sourcePayload,
            senderSignatureBase64: signature.base64EncodedString(),
            createdAtMs: unsignedReceipt.createdAtMs
        )
    }

    // MARK: - Canonical JSON — matches Iroha's canonical_json_bytes(CashTransferReceiptUnsignedPayload)

    /// Build unsigned payload bytes matching Rust's `cash_transfer_receipt_unsigned_payload`.
    ///
    /// Key rules from Android E2E + Rust source:
    /// - attestation: only 4 fields (norito skips optional ios_*/attestation_report when None)
    /// - authorization: CashTransferReceiptAuthorizationPayload with device_binding
    ///   (ios_* fields are omitted when nil)
    /// - source_payload: skip if nil
    /// - All keys sorted alphabetically at every nesting level
    private func buildCashUnsignedPayloadBytes(_ receipt: ToriiOfflineTransferReceipt) throws -> Data {
        // Attestation (4 fields only — norito skips optional fields)
        let attestObj: [String: Any] = [
            "assertion_base64": receipt.deviceProof.assertionBase64,
            "challenge_hash_hex": receipt.deviceProof.challengeHashHex,
            "counter": receipt.deviceProof.counter ?? 0,
            "key_id": receipt.deviceProof.attestationKeyId,
        ]

        // Authorization as CashTransferReceiptAuthorizationPayload
        var authObj: [String: Any]? = nil
        if let auth = receipt.authorization {
            let binding = auth.deviceBinding
            var bindingObj: [String: Any] = [
                "attestation_key_id": binding.attestationKeyId,
                "attestation_report_base64": binding.attestationReportBase64,
                "device_id": binding.deviceId,
                "offline_public_key": binding.offlinePublicKey,
                "platform": binding.platform,
            ]
            if let iosBundleId = binding.iosBundleId {
                bindingObj["ios_bundle_id"] = iosBundleId
            }
            if let iosEnvironment = binding.iosEnvironment {
                bindingObj["ios_environment"] = iosEnvironment
            }
            if let iosTeamId = binding.iosTeamId {
                bindingObj["ios_team_id"] = iosTeamId
            }

            authObj = [
                "account_id": auth.accountId,
                "authorization_id": auth.authorizationId,
                "device_binding": bindingObj,
                "expires_at_ms": auth.expiresAtMs,
                "issued_at_ms": auth.issuedAtMs,
                "issuer_signature_base64": auth.issuerSignatureBase64,
                "lineage_id": auth.lineageId,
                "max_balance": auth.policyMaxBalance,
                "max_tx_value": auth.policyMaxTxValue,
                "refresh_at_ms": auth.refreshAtMs,
                "verdict_id": auth.verdictId,
            ]
        }

        // Top-level payload
        var payload: [String: Any] = [
            "account_id": receipt.accountId,
            "amount": receipt.amount,
            "attestation": attestObj,
            "counterparty_account_id": receipt.counterpartyAccountId,
            "counterparty_device_id": receipt.counterpartyDeviceId,
            "counterparty_lineage_id": receipt.counterpartyLineageId,
            "counterparty_offline_public_key": receipt.counterpartyOfflinePublicKey,
            "created_at_ms": receipt.createdAtMs,
            "device_id": receipt.deviceId,
            "direction": receipt.direction.rawValue,
            "lineage_id": receipt.lineageId,
            "local_revision": receipt.localRevision,
            "offline_public_key": receipt.offlinePublicKey,
            "post_balance": receipt.postBalance,
            "post_locked_balance": receipt.postLockedBalance,
            "post_state_hash": receipt.postStateHash,
            "pre_balance": receipt.preBalance,
            "pre_locked_balance": receipt.preLockedBalance,
            "pre_state_hash": receipt.preStateHash,
            "transfer_id": receipt.transferId,
            "version": receipt.version,
        ]
        if let authObj { payload["authorization"] = authObj }
        if let sp = receipt.sourcePayload { payload["source_payload"] = sp }

        let jsonData = try JSONSerialization.data(
            withJSONObject: payload,
            options: [.sortedKeys, .withoutEscapingSlashes]
        )
        return jsonData
    }

    // MARK: - Source payload

    private func buildSourcePayload(
        senderState: ToriiOfflineCashState,
        outgoingReceipt: ToriiOfflineTransferReceipt
    ) throws -> String {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]
        let stateData = try encoder.encode(senderState)
        let receiptData = try encoder.encode(outgoingReceipt)

        var stateDict = try JSONSerialization.jsonObject(with: stateData) as! [String: Any]
        var receiptDict = try JSONSerialization.jsonObject(with: receiptData) as! [String: Any]

        // Enrich authorization with device_id, offline_public_key, app_attest_key_id
        // from device_binding (required by OfflineSpendAuthorization in Rust)
        enrichAuthorization(&stateDict)
        enrichAuthorization(&receiptDict)

        let wrapper: [String: Any] = [
            "version": 1,
            "anchor": stateDict,
            "ancestry_receipts": [] as [Any],
            "receipt": receiptDict,
        ]
        let data = try JSONSerialization.data(
            withJSONObject: wrapper,
            options: [.sortedKeys, .withoutEscapingSlashes]
        )
        return String(data: data, encoding: .utf8)!
    }

    /// Enrich authorization dict with device_id, offline_public_key, app_attest_key_id
    /// extracted from device_binding (Rust OfflineSpendAuthorization has these as top-level fields).
    private func enrichAuthorization(_ dict: inout [String: Any]) {
        guard var auth = dict["authorization"] as? [String: Any],
              let binding = auth["device_binding"] as? [String: Any] else { return }
        if auth["device_id"] == nil, let v = binding["device_id"] { auth["device_id"] = v }
        if auth["offline_public_key"] == nil, let v = binding["offline_public_key"] { auth["offline_public_key"] = v }
        if auth["app_attest_key_id"] == nil, let v = binding["attestation_key_id"] { auth["app_attest_key_id"] = v }
        dict["authorization"] = auth

        // Rename "device_proof" -> "attestation" and fix attestation field names
        // (Swift CodingKey vs Rust field names)
        if var dp = dict["device_proof"] as? [String: Any] {
            // Rename attestation_key_id -> key_id
            if let v = dp["attestation_key_id"] {
                dp["key_id"] = v
                dp.removeValue(forKey: "attestation_key_id")
            }
            // Rename challenge_hash_hex -> challenge_hash_hex (same)
            // Remove platform (not in OfflineDeviceAttestation)
            dp.removeValue(forKey: "platform")
            dict["attestation"] = dp
            dict.removeValue(forKey: "device_proof")
        }
    }

    // MARK: - API calls

    private func callSetup(
        accountId: String,
        identity: DeviceIdentity
    ) async throws -> ToriiOfflineCashEnvelope {
        let binding = makeDeviceBinding(identity: identity, includeAttestation: true)
        let payloadJSON = sortedJSON([
            "account_id": accountId,
            "device_id": identity.deviceId,
            "offline_public_key": identity.publicKeyBase64,
        ])
        let challengeHash = try computeChallengeHash(
            accountId: accountId,
            lineageId: "setup",
            operation: "setup",
            innerPayload: payloadJSON
        )
        let proof = try makeDeviceProof(identity: identity, challengeHashHex: challengeHash)
        let request = ToriiOfflineCashSetupRequest(
            accountId: accountId,
            assetDefinitionId: Self.assetDefinitionId,
            deviceBinding: binding,
            deviceProof: proof
        )
        return try await client.setupOfflineCash(request)
    }

    private func callLoad(
        accountId: String,
        lineageId: String,
        amount: String,
        identity: DeviceIdentity
    ) async throws -> ToriiOfflineCashEnvelope {
        let binding = makeDeviceBinding(identity: identity, includeAttestation: false)
        let effectiveLineageId = lineageId.isEmpty ? "setup" : lineageId
        let payloadJSON = sortedJSON([
            "amount": amount,
            "lineage_id": effectiveLineageId,
        ])
        let challengeHash = try computeChallengeHash(
            accountId: accountId,
            lineageId: effectiveLineageId,
            operation: "load",
            innerPayload: payloadJSON
        )
        let proof = try makeDeviceProof(identity: identity, challengeHashHex: challengeHash)
        let request = ToriiOfflineCashLoadRequest(
            operationId: UUID().uuidString,
            lineageId: lineageId,
            accountId: accountId,
            assetDefinitionId: Self.assetDefinitionId,
            amount: amount,
            deviceBinding: binding,
            deviceProof: proof
        )
        return try await client.loadOfflineCash(request)
    }

    private func callSync(
        accountId: String,
        lineageId: String,
        identity: DeviceIdentity,
        receipts: [ToriiOfflineTransferReceipt]
    ) async throws -> ToriiOfflineCashEnvelope {
        let binding = makeDeviceBinding(identity: identity, includeAttestation: false)
        let payloadJSON = sortedJSON(["lineage_id": lineageId])
        let challengeHash = try computeChallengeHash(
            accountId: accountId,
            lineageId: lineageId,
            operation: "sync",
            innerPayload: payloadJSON
        )
        let proof = try makeDeviceProof(identity: identity, challengeHashHex: challengeHash)
        let request = ToriiOfflineCashSyncRequest(
            operationId: UUID().uuidString,
            lineageId: lineageId,
            accountId: accountId,
            deviceBinding: binding,
            deviceProof: proof,
            receipts: receipts
        )
        return try await client.syncOfflineCash(request)
    }

    // MARK: - Helpers

    private func makeAccountId(publicKey: Data) throws -> String {
        let address = try AccountAddress.fromAccount(publicKey: publicKey, algorithm: "ed25519")
        return try address.toI105(networkPrefix: 0x02F1)
    }

    private func makeDeviceBinding(identity: DeviceIdentity, includeAttestation: Bool) -> ToriiOfflineDeviceBinding {
        ToriiOfflineDeviceBinding(
            platform: "ios",
            attestationKeyId: identity.attestationKeyId,
            deviceId: identity.deviceId,
            offlinePublicKey: identity.publicKeyBase64,
            attestationReportBase64: "",
            iosTeamId: nil,
            iosBundleId: nil,
            iosEnvironment: nil
        )
    }

    private func makeDeviceProof(
        identity: DeviceIdentity,
        challengeHashHex: String
    ) throws -> ToriiOfflineDeviceProof {
        let c = identity.nextCounter()
        return ToriiOfflineDeviceProof(
            platform: "ios",
            attestationKeyId: identity.attestationKeyId,
            challengeHashHex: challengeHashHex,
            assertionBase64: stubAssertionBase64(counter: c),
            counter: c
        )
    }

    /// Build a minimal valid CBOR assertion that the server can decode.
    /// Format: CBOR map { "authenticatorData": bytes(37), "signature": bytes(64) }
    private func stubAssertionBase64(counter: UInt64) -> String {
        var authData = Data(repeating: 0x00, count: 32)
        authData.append(0x01)
        let counterU32 = UInt32(counter & 0xFFFFFFFF)
        withUnsafeBytes(of: counterU32.bigEndian) { authData.append(contentsOf: $0) }

        var cbor = Data()
        cbor.append(0xA2) // map of 2 items
        cbor.append(0x71) // text(17)
        cbor.append(contentsOf: "authenticatorData".utf8)
        cbor.append(0x58)
        cbor.append(UInt8(authData.count))
        cbor.append(authData)
        cbor.append(0x69) // text(9)
        cbor.append(contentsOf: "signature".utf8)
        cbor.append(0x58)
        cbor.append(0x40) // 64
        cbor.append(Data(repeating: 0x00, count: 64))

        return cbor.base64EncodedString()
    }

    // MARK: - Challenge hash (mirrors Android computeChallengeHash)

    private func computeChallengeHash(
        accountId: String,
        lineageId: String,
        operation: String,
        innerPayload: String
    ) throws -> String {
        let payloadHash = sha256Hex(Data(innerPayload.utf8))
        let challengeSeed = sortedJSON([
            "account_id": accountId,
            "lineage_id": lineageId,
            "operation": operation,
            "payload_hash": payloadHash,
        ])
        return sha256Hex(Data(challengeSeed.utf8))
    }

    // MARK: - Local state hash

    private func computeLocalStateHash(
        lineageId: String,
        previousStateHash: String,
        transferId: String,
        direction: String,
        counterpartyLineageId: String,
        amount: String,
        localRevision: UInt64,
        postBalance: String,
        postLockedBalance: String
    ) throws -> String {
        // Build JSON with sorted keys matching Rust CashLocalStateHashPayload
        let json = try JSONSerialization.data(withJSONObject: [
            "amount": amount,
            "counterparty_lineage_id": counterpartyLineageId,
            "direction": direction,
            "lineage_id": lineageId,
            "local_revision": localRevision,
            "post_balance": postBalance,
            "post_locked_balance": postLockedBalance,
            "previous_state_hash": previousStateHash,
            "transfer_id": transferId,
        ] as [String: Any], options: [.sortedKeys, .withoutEscapingSlashes])
        return sha256Hex(json)
    }

    // MARK: - Account registration & minting

    private func registerAndFundAccount(accountId: String) async throws {
        // Register via Torii onboard (ignore conflict if already exists)
        let alias = "e2e-\(UUID().uuidString.prefix(8))@universal"
        let request = ToriiAccountOnboardingRequest(
            alias: alias,
            accountId: accountId
        )
        do {
            _ = try await client.registerAccount(request)
        } catch {
            // Ignore if already registered
            print("Registration note: \(error)")
        }

        // Wait for registration to commit
        try await Task.sleep(nanoseconds: 3_000_000_000)

        // Mint via CLI
        let process = Process()
        process.executableURL = mintContext.cliURL
        process.arguments = ["--config", mintContext.configURL.path, "ledger", "asset", "mint",
                             "--definition-alias", "usd#wonderland",
                             "--account", accountId,
                             "--quantity", "100"]
        let pipe = Pipe()
        process.standardOutput = pipe
        process.standardError = pipe
        try process.run()
        process.waitUntilExit()
        let output = String(data: pipe.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8) ?? ""
        print("Mint output: \(output) exit: \(process.terminationStatus)")

        // Wait for mint to commit
        try await Task.sleep(nanoseconds: 3_000_000_000)
    }

    private static func resolveMintContext() throws -> MintContext {
        let environment = ProcessInfo.processInfo.environment
        let fileManager = FileManager.default

        let cliURL: URL = {
            if let cliPath = environment[cliEnvKey]?.trimmingCharacters(in: .whitespacesAndNewlines),
               !cliPath.isEmpty {
                return URL(fileURLWithPath: cliPath)
            }
            return repositoryRoot.appendingPathComponent("target/release/iroha")
        }()
        guard fileManager.fileExists(atPath: cliURL.path) else {
            throw XCTSkip(
                "Iroha CLI not found at \(cliURL.path); set \(cliEnvKey) or build target/release/iroha"
            )
        }

        let configURL: URL = {
            if let configPath = environment[clientConfigEnvKey]?.trimmingCharacters(in: .whitespacesAndNewlines),
               !configPath.isEmpty {
                return URL(fileURLWithPath: configPath)
            }
            return repositoryRoot.appendingPathComponent("defaults/client.toml")
        }()
        guard fileManager.fileExists(atPath: configURL.path) else {
            throw XCTSkip(
                "Iroha client config not found at \(configURL.path); set \(clientConfigEnvKey) to a valid client.toml"
            )
        }

        let nodeWasOverridden = environment["IROHA_NODE_URL"]?.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty == false
        let configWasOverridden = environment[clientConfigEnvKey]?.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty == false
        if nodeWasOverridden && !configWasOverridden {
            let configContents = try String(contentsOf: configURL, encoding: .utf8)
            guard let configToriiURL = Self.extractToriiURL(from: configContents) else {
                throw XCTSkip("Could not read torii_url from \(configURL.path)")
            }
            if Self.normalizedURLString(configToriiURL) != Self.normalizedURLString(Self.nodeURL) {
                throw XCTSkip(
                    "\(clientConfigEnvKey) is not set, but defaults/client.toml points at \(configToriiURL.absoluteString) while IROHA_NODE_URL is \(Self.nodeURL.absoluteString)"
                )
            }
        }

        return MintContext(cliURL: cliURL, configURL: configURL)
    }

    private static func extractToriiURL(from configContents: String) -> URL? {
        for line in configContents.split(whereSeparator: \.isNewline) {
            let trimmed = line.trimmingCharacters(in: .whitespaces)
            guard trimmed.hasPrefix("torii_url"),
                  let separator = trimmed.firstIndex(of: "=") else {
                continue
            }
            let rawValue = trimmed[trimmed.index(after: separator)...]
                .trimmingCharacters(in: .whitespaces)
            guard rawValue.first == "\"", rawValue.last == "\"" else {
                continue
            }
            let start = rawValue.index(after: rawValue.startIndex)
            let end = rawValue.index(before: rawValue.endIndex)
            return URL(string: String(rawValue[start..<end]))
        }
        return nil
    }

    private static func normalizedURLString(_ url: URL) -> String {
        guard let components = URLComponents(url: url, resolvingAgainstBaseURL: false) else {
            return url.absoluteString.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
        }
        let scheme = components.scheme?.lowercased() ?? ""
        let host = components.host?.lowercased() ?? ""
        let port = components.port.map { ":\($0)" } ?? ""
        let path = components.path == "/" ? "" : components.path
        return "\(scheme)://\(host)\(port)\(path)"
    }

    // MARK: - Anchor hash (mirrors OfflineCashReceiptBuilder.computeAnchorHash)

    private func computeAnchorHash(state: ToriiOfflineCashState) throws -> String {
        let payload: [String: Any] = [
            "account_id": state.accountId,
            "asset_definition_id": state.assetDefinitionId,
            "authorization_id": state.authorization.authorizationId,
            "balance": canonicalAmount(Decimal(string: state.balance) ?? 0),
            "device_id": state.deviceId,
            "lineage_id": state.lineageId,
            "locked_balance": canonicalAmount(Decimal(string: state.lockedBalance) ?? 0),
            "offline_public_key": state.offlinePublicKey,
            "pending_local_revision": state.pendingLocalRevision,
            "server_revision": state.serverRevision,
        ]
        let data = try JSONSerialization.data(
            withJSONObject: payload,
            options: [.sortedKeys, .withoutEscapingSlashes]
        )
        return sha256Hex(data)
    }

    // MARK: - Utilities

    private func sortedJSON(_ dict: [String: String]) -> String {
        let pairs = dict.sorted { $0.key < $1.key }
            .map { "\"\($0.key)\":\"\($0.value)\"" }
        return "{\(pairs.joined(separator: ","))}"
    }

    private func sha256Hex(_ data: Data) -> String {
        SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
    }

    private func subtractAmounts(_ lhs: String, _ rhs: String) -> String {
        let left = Decimal(string: lhs) ?? 0
        let right = Decimal(string: rhs) ?? 0
        return canonicalAmount(left - right)
    }

    private func addAmounts(_ lhs: String, _ rhs: String) -> String {
        let left = Decimal(string: lhs) ?? 0
        let right = Decimal(string: rhs) ?? 0
        return canonicalAmount(left + right)
    }

    private func canonicalAmount(_ value: Decimal) -> String {
        var str = NSDecimalNumber(decimal: value).stringValue
        if str.contains(".") {
            while str.hasSuffix("0") { str.removeLast() }
            if str.hasSuffix(".") { str.removeLast() }
        }
        return str
    }
}

// MARK: - Data hex init

private extension Data {
    init(hex: String) {
        let chars = Array(hex)
        var bytes = [UInt8]()
        bytes.reserveCapacity(chars.count / 2)
        var i = chars.startIndex
        while i < chars.endIndex {
            let end = chars.index(i, offsetBy: 2, limitedBy: chars.endIndex) ?? chars.endIndex
            if let byte = UInt8(String(chars[i..<end]), radix: 16) {
                bytes.append(byte)
            }
            i = end
        }
        self.init(bytes)
    }
}
