import CryptoKit
import Foundation
import XCTest

@testable import IrohaSwift

final class OfflineReceiptBuilderTests: XCTestCase {
    private let chainId = "testnet"

    func testBuildSignedReceiptMatchesSignature() throws {
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 0x01, count: 32))
        let certificate = try makeCertificate(signingKey: signingKey)
        let issuedAtMs = validIssuedAtMs(for: certificate)
        let txId = IrohaHash.hash(Data("receipt-sign".utf8))
        let amount = "10.00"
        let invoiceId = "inv-1"
        let proof = try makeProof(txId: txId,
                                  receiver: certificate.controller,
                                  assetId: certificate.allowance.assetId,
                                  amount: amount,
                                  invoiceId: invoiceId,
                                  issuedAtMs: issuedAtMs)

        let receipt = try OfflineReceiptBuilder.buildSignedReceipt(
            txId: txId,
            chainId: chainId,
            receiverAccountId: certificate.controller,
            amount: amount,
            invoiceId: invoiceId,
            platformProof: proof,
            senderCertificate: certificate,
            signingKey: signingKey,
            issuedAtMs: issuedAtMs
        )

        let payload = try receipt.signingBytes()
        let publicKey = try Curve25519.Signing.PublicKey(rawRepresentation: signingKey.publicKey())
        XCTAssertTrue(publicKey.isValidSignature(receipt.senderSignature, for: payload))
        XCTAssertEqual(receipt.from, certificate.controller)
        XCTAssertEqual(receipt.assetId, certificate.allowance.assetId)
    }

    func testAggregateAmountRejectsFractionalAmounts() throws {
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 0x02, count: 32))
        let certificate = try makeCertificate(signingKey: signingKey)
        let issuedAtMs = validIssuedAtMs(for: certificate)
        let txId = IrohaHash.hash(Data("tx-frac".utf8))
        let proof = try makeProof(txId: txId,
                                  receiver: certificate.controller,
                                  assetId: certificate.allowance.assetId,
                                  amount: "1.0",
                                  invoiceId: "inv-frac",
                                  issuedAtMs: issuedAtMs)
        let receipt = OfflineSpendReceipt(
            txId: txId,
            from: certificate.controller,
            to: certificate.controller,
            assetId: certificate.allowance.assetId,
            amount: "1.0",
            issuedAtMs: issuedAtMs,
            invoiceId: "inv-frac",
            platformProof: proof,
            platformSnapshot: nil,
            senderCertificate: certificate,
            senderSignature: Data(repeating: 0xAB, count: 64)
        )

        XCTAssertThrowsError(
            try OfflineReceiptBuilder.aggregateAmount(receipts: [receipt])
        ) { error in
            XCTAssertEqual(error as? OfflineReceiptBuilderError,
                           .amountScaleMismatch(value: "1.0", expected: 2, actual: 1))
        }
    }

    func testAggregateAmountAcceptsScaledAmounts() throws {
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 0x03, count: 32))
        let base = try makeCertificate(signingKey: signingKey)
        let allowance = OfflineAllowanceCommitment(
            assetId: base.allowance.assetId,
            amount: "250.00",
            commitment: base.allowance.commitment
        )
        let policy = OfflineWalletPolicy(
            maxBalance: "500.00",
            maxTxValue: "125.00",
            expiresAtMs: base.policy.expiresAtMs
        )
        let certificate = OfflineWalletCertificate(
            controller: base.controller,
            allowance: allowance,
            spendPublicKey: base.spendPublicKey,
            attestationReport: base.attestationReport,
            issuedAtMs: base.issuedAtMs,
            expiresAtMs: base.expiresAtMs,
            policy: policy,
            operatorSignature: base.operatorSignature,
            metadata: base.metadata,
            verdictId: base.verdictId,
            attestationNonce: base.attestationNonce,
            refreshAtMs: base.refreshAtMs
        )
        let receipt = try makeReceipt(certificate: certificate,
                                      signingKey: signingKey,
                                      amount: "1.00",
                                      invoiceId: "inv-scale",
                                      seed: "scale")
        let total = try OfflineReceiptBuilder.aggregateAmount(receipts: [receipt])
        XCTAssertEqual(total, "1.00")
    }

    func testGeneratedIdsAreHashCompatible() {
        let receiptId = OfflineReceiptBuilder.generateReceiptId()
        XCTAssertEqual(receiptId.count, 32)
        XCTAssertEqual(receiptId.last.map { $0 & 1 }, 1)

        let bundleId = OfflineReceiptBuilder.generateBundleId()
        XCTAssertEqual(bundleId.count, 32)
        XCTAssertEqual(bundleId.last.map { $0 & 1 }, 1)

        let seed = Data("seeded".utf8)
        let seededReceipt = OfflineReceiptBuilder.generateReceiptId(seed: seed)
        let seededBundle = OfflineReceiptBuilder.generateBundleId(seed: seed)
        XCTAssertEqual(seededReceipt, OfflineReceiptBuilder.generateReceiptId(seed: seed))
        XCTAssertEqual(seededBundle, OfflineReceiptBuilder.generateBundleId(seed: seed))
        XCTAssertEqual(seededReceipt.count, 32)
        XCTAssertEqual(seededReceipt.last.map { $0 & 1 }, 1)
        XCTAssertEqual(seededBundle.count, 32)
        XCTAssertEqual(seededBundle.last.map { $0 & 1 }, 1)
    }

    func testComputeReplayLogTailMatchesManualHash() throws {
        let head = IrohaHash.hash(Data("replay-head".utf8))
        let txA = IrohaHash.hash(Data("replay-tx-a".utf8))
        let txB = IrohaHash.hash(Data("replay-tx-b".utf8))

        let tailHex = try OfflineReceiptBuilder.computeReplayLogTail(
            headHex: head.hexUppercased(),
            txIds: [txA, txB]
        )

        let domain = Data("iroha.offline.fastpq.replay.chain.v1".utf8)
        var current = head
        for txId in [txA, txB] {
            var writer = Data()
            writer.reserveCapacity(domain.count + current.count + txId.count)
            writer.append(domain)
            writer.append(current)
            writer.append(txId)
            current = IrohaHash.hash(writer)
        }

        XCTAssertEqual(tailHex, current.hexUppercased())
    }

    func testComputeReplayLogTailRejectsInvalidHeadHex() {
        XCTAssertThrowsError(
            try OfflineReceiptBuilder.computeReplayLogTail(headHex: "zz", txIds: [])
        ) { error in
            XCTAssertEqual(error as? OfflineReceiptBuilderError, .invalidReplayLogHeadHex)
        }
    }

    func testBuildSignedReceiptUsesSeededTxId() throws {
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 0x06, count: 32))
        let certificate = try makeCertificate(signingKey: signingKey)
        let issuedAtMs = validIssuedAtMs(for: certificate)
        let seed = Data("seeded-receipt".utf8)
        let txId = OfflineReceiptBuilder.generateReceiptId(seed: seed)
        let amount = "12.00"
        let invoiceId = "inv-seed"
        let proof = try makeProof(txId: txId,
                                  receiver: certificate.controller,
                                  assetId: certificate.allowance.assetId,
                                  amount: amount,
                                  invoiceId: invoiceId,
                                  issuedAtMs: issuedAtMs)

        let receipt = try OfflineReceiptBuilder.buildSignedReceipt(
            txIdSeed: seed,
            chainId: chainId,
            receiverAccountId: certificate.controller,
            amount: amount,
            invoiceId: invoiceId,
            platformProof: proof,
            senderCertificate: certificate,
            signingKey: signingKey,
            issuedAtMs: issuedAtMs
        )

        XCTAssertEqual(receipt.txId, txId)
    }

    func testBuildSignedReceiptRejectsSpendKeyMismatch() throws {
        let certificate = try OfflineWalletCertificate.load(from: fixtureURL("certificate.json"))
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 0x07, count: 32))
        let issuedAtMs = validIssuedAtMs(for: certificate)
        let expected = canonicalSpendKey("ed0120" + (try signingKey.publicKey()).hexUppercased())
        let actual = canonicalSpendKey(certificate.spendPublicKey)
        let txId = IrohaHash.hash(Data("spend-key-mismatch".utf8))
        let amount = "5.00"
        let invoiceId = "inv-mismatch"
        let proof = try makeProof(txId: txId,
                                  receiver: certificate.controller,
                                  assetId: certificate.allowance.assetId,
                                  amount: amount,
                                  invoiceId: invoiceId,
                                  issuedAtMs: issuedAtMs)

        XCTAssertThrowsError(
            try OfflineReceiptBuilder.buildSignedReceipt(
                txId: txId,
                chainId: chainId,
                receiverAccountId: certificate.controller,
                amount: amount,
                invoiceId: invoiceId,
                platformProof: proof,
                senderCertificate: certificate,
                signingKey: signingKey,
                issuedAtMs: issuedAtMs
            )
        ) { error in
            XCTAssertEqual(error as? OfflineReceiptBuilderError,
                           .spendKeyMismatch(expected: expected, actual: actual))
        }
    }

    func testBuildSignedReceiptRejectsInvalidReceiverAccountId() throws {
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 0x08, count: 32))
        let certificate = try makeCertificate(signingKey: signingKey)
        let issuedAtMs = validIssuedAtMs(for: certificate)
        let txId = IrohaHash.hash(Data("invalid-receiver".utf8))
        let proof = try makeProof(txId: txId,
                                  receiver: certificate.controller,
                                  assetId: certificate.allowance.assetId,
                                  amount: "9.00",
                                  invoiceId: "inv-invalid",
                                  issuedAtMs: issuedAtMs)

        XCTAssertThrowsError(
            try OfflineReceiptBuilder.buildSignedReceipt(
                txId: txId,
                chainId: chainId,
                receiverAccountId: "invalid",
                amount: "9.00",
                invoiceId: "inv-invalid",
                platformProof: proof,
                senderCertificate: certificate,
                signingKey: signingKey,
                issuedAtMs: issuedAtMs
            )
        ) { error in
            XCTAssertEqual(error as? OfflineReceiptBuilderError,
                           .invalidAccountId(field: "receiver", value: "invalid"))
        }
    }

    func testBuildSignedReceiptRejectsReservedAccountIdCharacters() throws {
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 0x09, count: 32))
        let certificate = try makeCertificate(signingKey: signingKey)
        let issuedAtMs = validIssuedAtMs(for: certificate)
        let txId = IrohaHash.hash(Data("reserved-account".utf8))
        let proof = try makeProof(txId: txId,
                                  receiver: certificate.controller,
                                  assetId: certificate.allowance.assetId,
                                  amount: "9.00",
                                  invoiceId: "inv-reserved",
                                  issuedAtMs: issuedAtMs)

        XCTAssertThrowsError(
            try OfflineReceiptBuilder.buildSignedReceipt(
                txId: txId,
                chainId: chainId,
                receiverAccountId: "alice$@wonderland",
                amount: "9.00",
                invoiceId: "inv-reserved",
                platformProof: proof,
                senderCertificate: certificate,
                signingKey: signingKey,
                issuedAtMs: issuedAtMs
            )
        ) { error in
            XCTAssertEqual(error as? OfflineReceiptBuilderError,
                           .invalidAccountId(field: "receiver", value: "alice$@wonderland"))
        }
    }

    func testBuildSignedReceiptRejectsWhitespaceAccountId() throws {
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 0x12, count: 32))
        let certificate = try makeCertificate(signingKey: signingKey)
        let issuedAtMs = validIssuedAtMs(for: certificate)
        let txId = IrohaHash.hash(Data("whitespace-account".utf8))
        let proof = try makeProof(txId: txId,
                                  receiver: certificate.controller,
                                  assetId: certificate.allowance.assetId,
                                  amount: "4.00",
                                  invoiceId: "inv-whitespace",
                                  issuedAtMs: issuedAtMs)

        XCTAssertThrowsError(
            try OfflineReceiptBuilder.buildSignedReceipt(
                txId: txId,
                chainId: chainId,
                receiverAccountId: "\(certificate.controller) ",
                amount: "4.00",
                invoiceId: "inv-whitespace",
                platformProof: proof,
                senderCertificate: certificate,
                signingKey: signingKey,
                issuedAtMs: issuedAtMs
            )
        ) { error in
            XCTAssertEqual(error as? OfflineReceiptBuilderError,
                           .invalidAccountId(field: "receiver", value: "\(certificate.controller)"))
        }
    }

    func testValidateReceiptRejectsInvalidSignature() throws {
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 0x0C, count: 32))
        let certificate = try makeCertificate(signingKey: signingKey)
        let receipt = try makeReceipt(certificate: certificate,
                                       signingKey: signingKey,
                                       amount: "3.00",
                                       invoiceId: "inv-bad-signature",
                                       seed: "tx-bad-sig")
        let tampered = OfflineSpendReceipt(
            txId: receipt.txId,
            from: receipt.from,
            to: receipt.to,
            assetId: receipt.assetId,
            amount: receipt.amount,
            issuedAtMs: receipt.issuedAtMs,
            invoiceId: receipt.invoiceId,
            platformProof: receipt.platformProof,
            platformSnapshot: receipt.platformSnapshot,
            senderCertificate: receipt.senderCertificate,
            senderSignature: Data(repeating: 0x00, count: receipt.senderSignature.count)
        )

        XCTAssertThrowsError(try OfflineReceiptBuilder.validateReceipt(tampered, chainId: chainId)) { error in
            XCTAssertEqual(error as? OfflineReceiptBuilderError, .invalidSenderSignature)
        }
    }

    func testBuildSignedReceiptRejectsSnapshotPolicyMismatch() throws {
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 0x0D, count: 32))
        var certificate = try makeCertificate(signingKey: signingKey)
        let issuedAtMs = validIssuedAtMs(for: certificate)
        certificate = OfflineWalletCertificate(
            controller: certificate.controller,
            allowance: certificate.allowance,
            spendPublicKey: certificate.spendPublicKey,
            attestationReport: certificate.attestationReport,
            issuedAtMs: certificate.issuedAtMs,
            expiresAtMs: certificate.expiresAtMs,
            policy: certificate.policy,
            operatorSignature: certificate.operatorSignature,
            metadata: ["android.integrity.policy": .string("hms_safety_detect")],
            verdictId: certificate.verdictId,
            attestationNonce: certificate.attestationNonce,
            refreshAtMs: certificate.refreshAtMs
        )
        let txId = IrohaHash.hash(Data("snapshot-policy".utf8))
        let proof = try makeProof(txId: txId,
                                  receiver: certificate.controller,
                                  assetId: certificate.allowance.assetId,
                                  amount: "8.00",
                                  invoiceId: "inv-snapshot",
                                  issuedAtMs: issuedAtMs)
        let snapshot = OfflinePlatformTokenSnapshot(
            policy: "play_integrity",
            attestationJwsB64: Data([0x01, 0x02]).base64EncodedString()
        )

        XCTAssertThrowsError(
            try OfflineReceiptBuilder.buildSignedReceipt(
                txId: txId,
                chainId: chainId,
                receiverAccountId: certificate.controller,
                amount: "8.00",
                invoiceId: "inv-snapshot",
                platformProof: proof,
                platformSnapshot: snapshot,
                senderCertificate: certificate,
                signingKey: signingKey,
                issuedAtMs: issuedAtMs
            )
        ) { error in
            XCTAssertEqual(error as? OfflineReceiptBuilderError,
                           .invalidPlatformSnapshot("platform token snapshot policy does not match allowance metadata"))
        }
    }

    func testBuildBalanceProofDerivesClaimedDelta() throws {
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 0x05, count: 32))
        let certificate = try makeCertificate(signingKey: signingKey)
        let receiptA = try makeReceipt(certificate: certificate,
                                       signingKey: signingKey,
                                       amount: "2.00",
                                       invoiceId: "inv-a",
                                       seed: "tx-a")
        let receiptB = try makeReceipt(certificate: certificate,
                                       signingKey: signingKey,
                                       amount: "3.00",
                                       invoiceId: "inv-b",
                                       seed: "tx-b")

        let balanceProof = try OfflineReceiptBuilder.buildBalanceProof(
            initialCommitment: certificate.allowance,
            resultingCommitment: Data(repeating: 0x33, count: 32),
            receipts: [receiptA, receiptB],
            zkProof: validZkProof()
        )

        XCTAssertEqual(balanceProof.claimedDelta, "5.00")
    }

    func testBuildTransferRejectsMissingBalanceProof() throws {
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 0x0B, count: 32))
        let certificate = try makeCertificate(signingKey: signingKey)
        let receipt = try makeReceipt(certificate: certificate,
                                      signingKey: signingKey,
                                      amount: "5.00",
                                      invoiceId: "inv-missing",
                                      seed: "tx-missing")
        let claimedDelta = try OfflineReceiptBuilder.aggregateAmount(receipts: [receipt])
        let balanceProof = OfflineBalanceProof(
            initialCommitment: certificate.allowance,
            resultingCommitment: Data(repeating: 0x10, count: 32),
            claimedDelta: claimedDelta,
            zkProof: nil
        )

        XCTAssertThrowsError(
            try OfflineReceiptBuilder.buildTransfer(
                chainId: chainId,
                receiver: certificate.controller,
                depositAccount: certificate.controller,
                receipts: [receipt],
                balanceProof: balanceProof
            )
        ) { error in
            XCTAssertEqual(error as? OfflineReceiptBuilderError, .missingBalanceProof)
        }
    }

    func testBuildTransferRejectsUnsupportedBalanceProofVersion() throws {
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 0x0C, count: 32))
        let certificate = try makeCertificate(signingKey: signingKey)
        let receipt = try makeReceipt(certificate: certificate,
                                      signingKey: signingKey,
                                      amount: "7.00",
                                      invoiceId: "inv-version",
                                      seed: "tx-version")
        let claimedDelta = try OfflineReceiptBuilder.aggregateAmount(receipts: [receipt])
        let balanceProof = OfflineBalanceProof(
            initialCommitment: certificate.allowance,
            resultingCommitment: Data(repeating: 0x20, count: 32),
            claimedDelta: claimedDelta,
            zkProof: validZkProof(version: 2)
        )

        XCTAssertThrowsError(
            try OfflineReceiptBuilder.buildTransfer(
                chainId: chainId,
                receiver: certificate.controller,
                depositAccount: certificate.controller,
                receipts: [receipt],
                balanceProof: balanceProof
            )
        ) { error in
            XCTAssertEqual(error as? OfflineReceiptBuilderError, .unsupportedBalanceProofVersion(2))
        }
    }

    func testBuildTransferRejectsClaimedDeltaMismatch() throws {
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 0x03, count: 32))
        let certificate = try makeCertificate(signingKey: signingKey)
        let receiptA = try makeReceipt(certificate: certificate,
                                       signingKey: signingKey,
                                       amount: "1.00",
                                       invoiceId: "inv-a",
                                       seed: "tx-a")
        let receiptB = try makeReceipt(certificate: certificate,
                                       signingKey: signingKey,
                                       amount: "2.00",
                                       invoiceId: "inv-b",
                                       seed: "tx-b")
        let balanceProof = OfflineBalanceProof(
            initialCommitment: certificate.allowance,
            resultingCommitment: Data(repeating: 0x11, count: 32),
            claimedDelta: "4.00",
            zkProof: validZkProof()
        )
        let bundleId = IrohaHash.hash(Data("bundle-id".utf8))

        XCTAssertThrowsError(
            try OfflineReceiptBuilder.buildTransfer(bundleId: bundleId,
                                                    chainId: chainId,
                                                    receiver: certificate.controller,
                                                    depositAccount: certificate.controller,
                                                    receipts: [receiptA, receiptB],
                                                    balanceProof: balanceProof)
        ) { error in
            XCTAssertEqual(error as? OfflineReceiptBuilderError,
                           .claimedDeltaMismatch(expected: "3.00", actual: "4.00"))
        }
    }

    func testBuildTransferRejectsDuplicateInvoiceId() throws {
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 0x04, count: 32))
        let certificate = try makeCertificate(signingKey: signingKey)
        let receiptA = try makeReceipt(certificate: certificate,
                                       signingKey: signingKey,
                                       amount: "5.00",
                                       invoiceId: "dup",
                                       seed: "tx-a")
        let receiptB = try makeReceipt(certificate: certificate,
                                       signingKey: signingKey,
                                       amount: "6.00",
                                       invoiceId: "dup",
                                       seed: "tx-b")
        let claimedDelta = try OfflineReceiptBuilder.aggregateAmount(receipts: [receiptA, receiptB])
        let balanceProof = OfflineBalanceProof(
            initialCommitment: certificate.allowance,
            resultingCommitment: Data(repeating: 0x22, count: 32),
            claimedDelta: claimedDelta,
            zkProof: validZkProof()
        )

        XCTAssertThrowsError(
            try OfflineReceiptBuilder.buildTransfer(chainId: chainId,
                                                    receiver: certificate.controller,
                                                    depositAccount: certificate.controller,
                                                    receipts: [receiptA, receiptB],
                                                    balanceProof: balanceProof)
        ) { error in
            XCTAssertEqual(error as? OfflineReceiptBuilderError,
                           .duplicateInvoiceId("dup"))
        }
    }

    func testBuildTransferUsesBundleIdSeed() throws {
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 0x09, count: 32))
        let certificate = try makeCertificate(signingKey: signingKey)
        let receipt = try makeReceipt(certificate: certificate,
                                      signingKey: signingKey,
                                      amount: "4.00",
                                      invoiceId: "inv-seeded",
                                      seed: "tx-seeded")
        let claimedDelta = try OfflineReceiptBuilder.aggregateAmount(receipts: [receipt])
        let balanceProof = OfflineBalanceProof(
            initialCommitment: certificate.allowance,
            resultingCommitment: Data(repeating: 0x44, count: 32),
            claimedDelta: claimedDelta,
            zkProof: validZkProof()
        )
        let seed = Data("bundle-seed".utf8)
        let expected = OfflineReceiptBuilder.generateBundleId(seed: seed)

        let transfer = try OfflineReceiptBuilder.buildTransfer(
            bundleIdSeed: seed,
            chainId: chainId,
            receiver: certificate.controller,
            depositAccount: certificate.controller,
            receipts: [receipt],
            balanceProof: balanceProof
        )

        XCTAssertEqual(transfer.bundleId, expected)
    }

    func testBuildTransferSortsReceipts() throws {
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 0x0A, count: 32))
        let certificate = try makeCertificate(signingKey: signingKey)
        let receiptHigh = try makeReceipt(certificate: certificate,
                                          signingKey: signingKey,
                                          amount: "2.00",
                                          invoiceId: "inv-high",
                                          seed: "tx-high",
                                          counter: 2)
        let receiptLow = try makeReceipt(certificate: certificate,
                                         signingKey: signingKey,
                                         amount: "1.00",
                                         invoiceId: "inv-low",
                                         seed: "tx-low",
                                         counter: 1)
        let claimedDelta = try OfflineReceiptBuilder.aggregateAmount(receipts: [receiptHigh, receiptLow])
        let balanceProof = OfflineBalanceProof(
            initialCommitment: certificate.allowance,
            resultingCommitment: Data(repeating: 0x55, count: 32),
            claimedDelta: claimedDelta,
            zkProof: validZkProof()
        )

        let transfer = try OfflineReceiptBuilder.buildTransfer(
            chainId: chainId,
            receiver: certificate.controller,
            depositAccount: certificate.controller,
            receipts: [receiptHigh, receiptLow],
            balanceProof: balanceProof,
            sortReceipts: true
        )

        XCTAssertEqual(transfer.receipts.first?.txId, receiptLow.txId)
        XCTAssertEqual(transfer.receipts.last?.txId, receiptHigh.txId)
    }

    func testBuildTransferRejectsUnsortedReceipts() throws {
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 0x0B, count: 32))
        let certificate = try makeCertificate(signingKey: signingKey)
        let receiptHigh = try makeReceipt(certificate: certificate,
                                          signingKey: signingKey,
                                          amount: "2.00",
                                          invoiceId: "inv-high",
                                          seed: "tx-high",
                                          counter: 2)
        let receiptLow = try makeReceipt(certificate: certificate,
                                         signingKey: signingKey,
                                         amount: "1.00",
                                         invoiceId: "inv-low",
                                         seed: "tx-low",
                                         counter: 1)
        let claimedDelta = try OfflineReceiptBuilder.aggregateAmount(receipts: [receiptHigh, receiptLow])
        let balanceProof = OfflineBalanceProof(
            initialCommitment: certificate.allowance,
            resultingCommitment: Data(repeating: 0x57, count: 32),
            claimedDelta: claimedDelta,
            zkProof: validZkProof()
        )

        XCTAssertThrowsError(
            try OfflineReceiptBuilder.buildTransfer(
                chainId: chainId,
                receiver: certificate.controller,
                depositAccount: certificate.controller,
                receipts: [receiptHigh, receiptLow],
                balanceProof: balanceProof,
                sortReceipts: false
            )
        ) { error in
            XCTAssertEqual(error as? OfflineReceiptBuilderError, .receiptOrderInvalid)
        }
    }

    func testBuildTransferRejectsMixedCounterScopes() throws {
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 0x0C, count: 32))
        let certificate = try makeCertificate(signingKey: signingKey)
        let appleReceipt = try makeReceipt(certificate: certificate,
                                           signingKey: signingKey,
                                           amount: "1.00",
                                           invoiceId: "inv-apple",
                                           seed: "tx-apple",
                                           counter: 1)
        let markerPublicKey = Data([0x04] + Array(repeating: 0x11, count: 64))
        let markerSeries = "marker::" + IrohaHash.hash(markerPublicKey).hexLowercased()
        let markerProof = AndroidMarkerKeyProof(series: markerSeries,
                                                counter: 2,
                                                markerPublicKey: markerPublicKey,
                                                markerSignature: nil,
                                                attestation: Data([0x01]))
        let markerReceipt = try OfflineReceiptBuilder.buildSignedReceipt(
            chainId: chainId,
            receiverAccountId: certificate.controller,
            amount: "2.00",
            invoiceId: "inv-marker",
            platformProof: .androidMarkerKey(markerProof),
            senderCertificate: certificate,
            signingKey: signingKey,
            issuedAtMs: validIssuedAtMs(for: certificate)
        )
        let claimedDelta = try OfflineReceiptBuilder.aggregateAmount(receipts: [appleReceipt, markerReceipt])
        let balanceProof = OfflineBalanceProof(
            initialCommitment: certificate.allowance,
            resultingCommitment: Data(repeating: 0x58, count: 32),
            claimedDelta: claimedDelta,
            zkProof: validZkProof()
        )

        XCTAssertThrowsError(
            try OfflineReceiptBuilder.buildTransfer(
                chainId: chainId,
                receiver: certificate.controller,
                depositAccount: certificate.controller,
                receipts: [appleReceipt, markerReceipt],
                balanceProof: balanceProof
            )
        ) { error in
            XCTAssertEqual(error as? OfflineReceiptBuilderError, .mixedCounterScopes)
        }
    }

    func testBuildTransferValidatesAggregateProofRoot() throws {
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 0x0E, count: 32))
        let certificate = try makeCertificate(signingKey: signingKey)
        let receipt = try makeReceipt(certificate: certificate,
                                      signingKey: signingKey,
                                      amount: "5.00",
                                      invoiceId: "inv-root",
                                      seed: "tx-root")
        let claimedDelta = try OfflineReceiptBuilder.aggregateAmount(receipts: [receipt])
        let balanceProof = OfflineBalanceProof(
            initialCommitment: certificate.allowance,
            resultingCommitment: Data(repeating: 0x66, count: 32),
            claimedDelta: claimedDelta,
            zkProof: validZkProof()
        )
        let root = try OfflineReceiptBuilder.computeReceiptsRoot(receipts: [receipt])
        let envelope = OfflineAggregateProofEnvelope(version: 1, receiptsRoot: root)
        let transfer = try OfflineReceiptBuilder.buildTransfer(
            chainId: chainId,
            receiver: certificate.controller,
            depositAccount: certificate.controller,
            receipts: [receipt],
            balanceProof: balanceProof,
            aggregateProof: envelope
        )
        XCTAssertEqual(transfer.aggregateProof?.receiptsRoot, root)

        let invalidEnvelope = OfflineAggregateProofEnvelope(
            version: 1,
            receiptsRoot: OfflinePoseidonDigest(bytes: Data(repeating: 0xAA, count: 32))
        )
        XCTAssertThrowsError(
            try OfflineReceiptBuilder.buildTransfer(
                chainId: chainId,
                receiver: certificate.controller,
                depositAccount: certificate.controller,
                receipts: [receipt],
                balanceProof: balanceProof,
                aggregateProof: invalidEnvelope
            )
        ) { error in
            XCTAssertEqual(error as? OfflineReceiptBuilderError, .aggregateProofRootMismatch)
        }
    }

    func testBuildAggregateProofEnvelopeUsesReceiptsRoot() throws {
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 0x0F, count: 32))
        let certificate = try makeCertificate(signingKey: signingKey)
        let receipt = try makeReceipt(certificate: certificate,
                                      signingKey: signingKey,
                                      amount: "5.00",
                                      invoiceId: "inv-envelope",
                                      seed: "tx-envelope")
        let envelope = try OfflineReceiptBuilder.buildAggregateProofEnvelope(
            receipts: [receipt],
            proofSum: Data([0x01]),
            metadata: [
                OfflineAggregateProofMetadataKey.parameterSet: .string("fastpq-offline-v1"),
                OfflineAggregateProofMetadataKey.sumCircuit: .string("fastpq/offline_sum/v1"),
            ]
        )
        let root = try OfflineReceiptBuilder.computeReceiptsRoot(receipts: [receipt])
        XCTAssertEqual(envelope.receiptsRoot, root)
        XCTAssertEqual(envelope.proofSum, Data([0x01]))

        XCTAssertThrowsError(
            try OfflineReceiptBuilder.buildAggregateProofEnvelope(receipts: [receipt], version: 2)
        ) { error in
            XCTAssertEqual(error as? OfflineReceiptBuilderError, .aggregateProofVersionUnsupported(2))
        }
    }

    func testGenerateAggregateProofsReturnsNilWhenNoRequests() throws {
        let proofs = try OfflineReceiptBuilder.generateAggregateProofs()
        XCTAssertNil(proofs.sum)
        XCTAssertNil(proofs.counter)
        XCTAssertNil(proofs.replay)
    }

    func testGenerateAggregateProofsReportsBridgeFailure() throws {
        let request = ToriiJSONValue.object(["invalid": .string("payload")])
        XCTAssertThrowsError(
            try OfflineReceiptBuilder.generateAggregateProofs(sumRequest: request)
        ) { error in
            guard let error = error as? OfflineReceiptBuilderError else {
                return XCTFail("Unexpected error type: \(error)")
            }
            switch error {
            case .aggregateProofGenerationUnavailable,
                 .aggregateProofGenerationFailed:
                break
            default:
                XCTFail("Unexpected error case: \(error)")
            }
        }
    }

    func testAggregateProofFixtureMatchesReceiptsRoot() throws {
        let fixture = try loadAggregateProofFixture()
        let certificate = try OfflineWalletCertificate.load(from: fixtureURL("certificate.json"))
        let receipts = try fixture.receipts.map { entry in
            try aggregateFixtureReceipt(entry, certificate: certificate)
        }
        let root = try OfflineReceiptBuilder.computeReceiptsRoot(receipts: receipts)
        XCTAssertEqual(root.bytes.hexUppercased(), fixture.receiptsRootHex)

        let proofSum = try decodeHexOptional(fixture.proofSumHex, field: "proof_sum_hex")
        let proofCounter = try decodeHexOptional(fixture.proofCounterHex, field: "proof_counter_hex")
        let proofReplay = try decodeHexOptional(fixture.proofReplayHex, field: "proof_replay_hex")
        let metadata: [String: ToriiJSONValue] = fixture.metadata.reduce(into: [:]) { result, entry in
            result[entry.key] = .string(entry.value)
        }
        let envelope = try OfflineReceiptBuilder.buildAggregateProofEnvelope(
            receipts: receipts,
            proofSum: proofSum,
            proofCounter: proofCounter,
            proofReplay: proofReplay,
            metadata: metadata
        )

        XCTAssertEqual(envelope.receiptsRoot.bytes.hexUppercased(), fixture.receiptsRootHex)
        XCTAssertEqual(envelope.proofSum, proofSum)
        XCTAssertEqual(envelope.proofCounter, proofCounter)
        XCTAssertEqual(envelope.proofReplay, proofReplay)
        XCTAssertEqual(envelope.metadata, metadata)
    }

    func testComputeReceiptsRootMatchesPoseidonVector() throws {
        let receiptA = try makePoseidonSampleReceipt(seed: "a", counter: 5)
        let receiptB = try makePoseidonSampleReceipt(seed: "b", counter: 6)
        XCTAssertEqual(receiptA.to,
                       "34mSYnDgbaJM58rbLoif4Tkp7G6HiNtnFxyXXKs7h64Y4tu5ce2JmpnsmfGYB7ZMkKaTZhYQF")
        XCTAssertEqual(receiptB.to,
                       "34mSYnDgbaJM58rbLoif4Tkp7G6HiNtnFxyXXKs7h64Y4tu5ce2JmpnsmfGYB7ZMkKaTZhYQF")
        XCTAssertEqual(receiptA.txId.hexUppercased(),
                       "CBC9BA205005B3C4801668402BE63A9CF861473E6619DB0C97BA185A65EB1457")
        XCTAssertEqual(receiptB.txId.hexUppercased(),
                       "9268B995F2034032820A0A8C5DBA0C2BDBD301F5130F1D7F8CBE28305E32088B")
        let fieldsA = try poseidonLeafFields(receiptA)
        let fieldsB = try poseidonLeafFields(receiptB)
        XCTAssertEqual(fieldsA.receiverHash, "3B3E4FCF62BBEBD4203FE301A90AC8062916DB9F31E6906A4530EB5669C018F3")
        XCTAssertEqual(fieldsB.receiverHash, "3B3E4FCF62BBEBD4203FE301A90AC8062916DB9F31E6906A4530EB5669C018F3")
        XCTAssertEqual(fieldsA.invoiceHash, "E00BE9DF519D103B25A6D2A1DB7580E50F8373A00F36DDCB0702BC89EE0AD1DD")
        XCTAssertEqual(fieldsB.invoiceHash, "989A58422C86F0BD2FEADF2EEB4648D532402E003DEEC9A7D9C932BB7FD6A779")
        XCTAssertEqual(fieldsA.platformProofHash, "F7B9AC9BC9CDE2A7BE9002A1F36E5FD7CD11BF07FA2D793CAA50554A99A389A3")
        XCTAssertEqual(fieldsB.platformProofHash, "4286178ABF3C0AF3FB9DC9E8389167F2369767087F9944EF90E8E615EA1DF3D7")
        let root = try OfflineReceiptBuilder.computeReceiptsRoot(receipts: [receiptA, receiptB])
        XCTAssertEqual(root.bytes.hexUppercased(),
                       "000000000000000000000000000000000000000000000000092966700D9AD6BB")
    }

    func testReceiptRecorderAppendsAuditAndJournal() throws {
        let signingKey = try SigningKey.ed25519(privateKey: Data(repeating: 0x0B, count: 32))
        let certificate = try makeCertificate(signingKey: signingKey)
        let journal = try OfflineJournal(url: temporaryJournalURL(), key: journalKey())
        let logger = try OfflineAuditLogger(storageURL: temporaryLogURL(), isEnabled: true)
        let recorder = OfflineReceiptRecorder(journal: journal, auditLogger: logger)
        let txId = IrohaHash.hash(Data("recorder".utf8))
        let amount = "7.00"
        let invoiceId = "inv-recorder"
        let issuedAtMs = validIssuedAtMs(for: certificate)
        let proof = try makeProof(txId: txId,
                                  receiver: certificate.controller,
                                  assetId: certificate.allowance.assetId,
                                  amount: amount,
                                  invoiceId: invoiceId,
                                  issuedAtMs: issuedAtMs)

        let receipt = try OfflineReceiptBuilder.buildSignedReceipt(
            txId: txId,
            chainId: chainId,
            receiverAccountId: certificate.controller,
            amount: amount,
            invoiceId: invoiceId,
            platformProof: proof,
            senderCertificate: certificate,
            signingKey: signingKey,
            recorder: recorder,
            timestampMs: 987,
            issuedAtMs: issuedAtMs
        )

        let pending = journal.pendingEntries()
        XCTAssertEqual(pending.count, 1)
        XCTAssertEqual(pending.first?.txId, receipt.txId)

        let entries = waitForAuditEntries(logger, count: 1)
        let entry = try XCTUnwrap(entries.first)
        XCTAssertEqual(entry.txId, receipt.txId.hexUppercased())
        XCTAssertEqual(entry.timestampMs, 987)
    }

    // MARK: - Helpers

    private func makeCertificate(signingKey: SigningKey) throws -> OfflineWalletCertificate {
        let base = try OfflineWalletCertificate.load(from: fixtureURL("certificate.json"))
        let spendKey = "ed0120" + (try signingKey.publicKey()).hexUppercased()
        return OfflineWalletCertificate(
            controller: base.controller,
            allowance: base.allowance,
            spendPublicKey: spendKey,
            attestationReport: base.attestationReport,
            issuedAtMs: base.issuedAtMs,
            expiresAtMs: base.expiresAtMs,
            policy: base.policy,
            operatorSignature: base.operatorSignature,
            metadata: base.metadata,
            verdictId: base.verdictId,
            attestationNonce: base.attestationNonce,
            refreshAtMs: base.refreshAtMs
        )
    }

    private func makeReceipt(certificate: OfflineWalletCertificate,
                             signingKey: SigningKey,
                             amount: String,
                             invoiceId: String,
                             seed: String,
                             counter: UInt64 = 1) throws -> OfflineSpendReceipt {
        let issuedAtMs = validIssuedAtMs(for: certificate)
        let txId = IrohaHash.hash(Data(seed.utf8))
        let proof = try makeProof(txId: txId,
                                  receiver: certificate.controller,
                                  assetId: certificate.allowance.assetId,
                                  amount: amount,
                                  invoiceId: invoiceId,
                                  issuedAtMs: issuedAtMs,
                                  counter: counter)
        return try OfflineReceiptBuilder.buildSignedReceipt(
            txId: txId,
            chainId: chainId,
            receiverAccountId: certificate.controller,
            amount: amount,
            invoiceId: invoiceId,
            platformProof: proof,
            senderCertificate: certificate,
            signingKey: signingKey,
            issuedAtMs: issuedAtMs
        )
    }

    private func validIssuedAtMs(for certificate: OfflineWalletCertificate,
                                 offset: UInt64 = 1_000) -> UInt64 {
        let expiryBound = min(certificate.expiresAtMs, certificate.policy.expiresAtMs)
        let candidate = certificate.issuedAtMs &+ offset
        return candidate > expiryBound ? expiryBound : candidate
    }

    private func makeProof(txId: Data,
                           receiver: String,
                           assetId: String,
                           amount: String,
                           invoiceId: String,
                           issuedAtMs: UInt64 = 1_700_000_000_000,
                           counter: UInt64 = 1) throws -> OfflinePlatformProof {
        let challenge = try challengeHash(txId: txId,
                                          chainId: chainId,
                                          receiver: receiver,
                                          assetId: assetId,
                                          amount: amount,
                                          issuedAtMs: issuedAtMs,
                                          invoiceId: invoiceId)
        let keyId = Data("swift-tests".utf8).base64EncodedString()
        let proof = AppleAppAttestProof(keyId: keyId,
                                        counter: counter,
                                        assertion: Data([0xAA]),
                                        challengeHash: challenge)
        return .appleAppAttest(proof)
    }

    private func challengeHash(txId: Data,
                               chainId: String,
                               receiver: String,
                               assetId: String,
                               amount: String,
                               issuedAtMs: UInt64,
                               invoiceId: String) throws -> Data {
        let preimage = OfflineReceiptChallengePreimage(
            invoiceId: invoiceId,
            receiverAccountId: receiver,
            assetId: assetId,
            amount: amount,
            issuedAtMs: issuedAtMs,
            nonceHex: txId.hexUppercased()
        )
        let payload = try preimage.noritoPayload()
        let bytes = OfflineNorito.wrap(typeName: OfflineReceiptChallengePreimage.noritoTypeName,
                                       payload: payload)
        let context = IrohaHash.hash(Data(chainId.utf8))
        var hashInput = Data()
        hashInput.reserveCapacity(context.count + bytes.count)
        hashInput.append(context)
        hashInput.append(bytes)
        return IrohaHash.hash(hashInput)
    }

    private func canonicalSpendKey(_ value: String) -> String {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        let raw: String
        if let separator = trimmed.firstIndex(of: ":") {
            raw = String(trimmed[trimmed.index(after: separator)...])
        } else {
            raw = trimmed
        }
        let upper = raw.uppercased()
        if upper.count == 64, Data(hexString: upper) != nil {
            return "ED0120" + upper
        }
        return upper
    }

    private func validZkProof(version: UInt8 = 1) -> Data {
        var proof = Data(repeating: 0, count: OfflineBalanceProofBuilder.proofLength)
        proof[0] = version
        return proof
    }

    private func makePoseidonSampleReceipt(seed: String, counter: UInt64) throws -> OfflineSpendReceipt {
        let controller = try sampleAccountId()
        let assetId = "usd#wonderland#\(controller)"
        let spendKey = try SigningKey.ed25519(privateKey: Data(repeating: 0x02, count: 32))
        let certificate = OfflineWalletCertificate(
            controller: controller,
            allowance: OfflineAllowanceCommitment(
                assetId: assetId,
                amount: "1000",
                commitment: Data(repeating: 0, count: 32)
            ),
            spendPublicKey: "ed0120" + (try spendKey.publicKey()).hexUppercased(),
            attestationReport: Data(),
            issuedAtMs: 0,
            expiresAtMs: 1,
            policy: OfflineWalletPolicy(maxBalance: "10000", maxTxValue: "1000", expiresAtMs: 1),
            operatorSignature: Data(repeating: 0xAB, count: 64),
            metadata: [:],
            verdictId: nil,
            attestationNonce: nil,
            refreshAtMs: nil
        )
        let txId = IrohaHash.hash(Data("tx-\(seed)".utf8))
        let amount = "250"
        let invoiceId = "invoice-\(seed)"
        let issuedAtMs: UInt64 = 1
        let challengeHash = IrohaHash.hash(Data("challenge".utf8))
        let proof = AppleAppAttestProof(
            keyId: seed,
            counter: counter,
            assertion: Data([1, 2, 3]),
            challengeHash: challengeHash
        )
        return OfflineSpendReceipt(
            txId: txId,
            from: controller,
            to: controller,
            assetId: assetId,
            amount: amount,
            issuedAtMs: issuedAtMs,
            invoiceId: invoiceId,
            platformProof: .appleAppAttest(proof),
            platformSnapshot: nil,
            senderCertificate: certificate,
            senderSignature: Data(repeating: 0xCD, count: 64)
        )
    }

    private func sampleAccountId() throws -> String {
        // Matches iroha_crypto::KeyPair::from_seed([0x01; 32], Ed25519).
        guard let publicKey = Data(hexString: "8A88E3DD7409F195FD52DB2D3CBA5D72CA6709BF1D94121BF3748801B40F6F5C") else {
            throw OfflineReceiptBuilderError.invalidAccountId(field: "receiver", value: "sample-key")
        }
        let address = try AccountAddress.fromAccount(domain: "wonderland",
                                                     publicKey: publicKey)
        let ih58 = try address.toIH58(networkPrefix: 0x02F1)
        return ih58
    }

    private func waitForAuditEntries(_ logger: OfflineAuditLogger,
                                     count: Int,
                                     timeout: TimeInterval = 0.5) -> [OfflineAuditEntry] {
        let deadline = Date().addingTimeInterval(timeout)
        var entries = logger.exportEntries()
        while entries.count < count && Date() < deadline {
            RunLoop.current.run(mode: .default, before: Date().addingTimeInterval(0.01))
            entries = logger.exportEntries()
        }
        return entries
    }

    private func temporaryJournalURL() -> URL {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".ijournal")
        addTeardownBlock {
            try? FileManager.default.removeItem(at: url)
        }
        return url
    }

    private func temporaryLogURL() -> URL {
        let url = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".json")
        addTeardownBlock {
            try? FileManager.default.removeItem(at: url)
        }
        return url
    }

    private func journalKey() -> OfflineJournalKey {
        OfflineJournalKey.derive(from: Data("receipt-recorder-key".utf8))
    }

    private func poseidonLeafFields(_ receipt: OfflineSpendReceipt) throws -> (
        receiverHash: String,
        invoiceHash: String,
        platformProofHash: String
    ) {
        let receiverPayload = try OfflineNorito.encodeAccountId(receipt.to)
        let receiverBytes = OfflineNorito.wrap(
            typeName: "iroha_data_model::account::model::AccountId",
            payload: receiverPayload
        )
        let receiverHash = IrohaHash.hash(receiverBytes)
        let invoiceHash = IrohaHash.hash(Data(receipt.invoiceId.utf8))
        let proofPayload = try receipt.platformProof.noritoPayload()
        let proofBytes = OfflineNorito.wrap(
            typeName: "iroha_data_model::offline::model::OfflinePlatformProof",
            payload: proofPayload
        )
        let proofHash = IrohaHash.hash(proofBytes)
        return (
            receiverHash: receiverHash.hexUppercased(),
            invoiceHash: invoiceHash.hexUppercased(),
            platformProofHash: proofHash.hexUppercased()
        )
    }

    private func fixtureURL(_ name: String) -> URL {
        var url = URL(fileURLWithPath: #filePath)
        for _ in 0..<4 {
            url.deleteLastPathComponent()
        }
        return url.appendingPathComponent("fixtures/offline_allowance/ios-demo/\(name)")
    }

    private func aggregateFixtureURL() -> URL {
        var url = URL(fileURLWithPath: #filePath)
        for _ in 0..<4 {
            url.deleteLastPathComponent()
        }
        return url.appendingPathComponent("fixtures/offline_bundle/aggregate_proof_fixture.json")
    }

    private func loadAggregateProofFixture() throws -> AggregateProofFixture {
        let data = try Data(contentsOf: aggregateFixtureURL())
        return try JSONDecoder().decode(AggregateProofFixture.self, from: data)
    }

    private func aggregateFixtureReceipt(
        _ receipt: AggregateProofFixture.Receipt,
        certificate: OfflineWalletCertificate
    ) throws -> OfflineSpendReceipt {
        let txId = try decodeFixtureHash(receipt.txIdHex, field: "tx_id_hex")
        let challengeHash = try decodeFixtureHash(receipt.platformProof.challengeHashHex,
                                                  field: "challenge_hash_hex")
        guard let assertion = Data(base64Encoded: receipt.platformProof.assertionB64) else {
            throw AggregateProofFixtureError.invalidBase64("assertion_b64")
        }
        guard receipt.platformProof.kind == "apple_app_attest" else {
            throw AggregateProofFixtureError.unsupportedPlatformProof(receipt.platformProof.kind)
        }
        let proof = OfflinePlatformProof.appleAppAttest(
            AppleAppAttestProof(
                keyId: receipt.platformProof.keyId,
                counter: receipt.platformProof.counter,
                assertion: assertion,
                challengeHash: challengeHash
            )
        )
        return OfflineSpendReceipt(
            txId: txId,
            from: receipt.from,
            to: receipt.to,
            assetId: receipt.asset,
            amount: receipt.amount,
            issuedAtMs: receipt.issuedAtMs,
            invoiceId: receipt.invoiceId,
            platformProof: proof,
            platformSnapshot: nil,
            senderCertificate: certificate,
            senderSignature: Data(repeating: 0, count: 64)
        )
    }

    private func decodeHexOptional(_ value: String?, field: String) throws -> Data? {
        guard let value = value else {
            return nil
        }
        guard let data = Data(hexString: value) else {
            throw AggregateProofFixtureError.invalidHex(field)
        }
        return data
    }

    private func decodeFixtureHash(_ value: String, field: String) throws -> Data {
        guard var data = Data(hexString: value), data.count == 32 else {
            throw AggregateProofFixtureError.invalidHex(field)
        }
        if let lastIndex = data.indices.last {
            data[lastIndex] |= 1
        }
        return data
    }

    private enum AggregateProofFixtureError: Error {
        case invalidHex(String)
        case invalidBase64(String)
        case unsupportedPlatformProof(String)
    }

    private struct AggregateProofFixture: Decodable {
        let receiptsRootHex: String
        let proofSumHex: String?
        let proofCounterHex: String?
        let proofReplayHex: String?
        let metadata: [String: String]
        let receipts: [Receipt]

        private enum CodingKeys: String, CodingKey {
            case receiptsRootHex = "receipts_root_hex"
            case proofSumHex = "proof_sum_hex"
            case proofCounterHex = "proof_counter_hex"
            case proofReplayHex = "proof_replay_hex"
            case metadata
            case receipts
        }

        struct Receipt: Decodable {
            let txIdHex: String
            let from: String
            let to: String
            let asset: String
            let amount: String
            let issuedAtMs: UInt64
            let invoiceId: String
            let platformProof: PlatformProof

            private enum CodingKeys: String, CodingKey {
                case txIdHex = "tx_id_hex"
                case from
                case to
                case asset
                case amount
                case issuedAtMs = "issued_at_ms"
                case invoiceId = "invoice_id"
                case platformProof = "platform_proof"
            }
        }

        struct PlatformProof: Decodable {
            let kind: String
            let keyId: String
            let counter: UInt64
            let assertionB64: String
            let challengeHashHex: String

            private enum CodingKeys: String, CodingKey {
                case kind
                case keyId = "key_id"
                case counter
                case assertionB64 = "assertion_b64"
                case challengeHashHex = "challenge_hash_hex"
            }
        }
    }
}
