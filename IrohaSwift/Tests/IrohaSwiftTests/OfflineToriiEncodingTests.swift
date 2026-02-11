import Foundation
import XCTest

@testable import IrohaSwift

final class OfflineToriiEncodingTests: XCTestCase {
    func testSpendReceiptToriiJSONEncoding() throws {
        let certificate = try OfflineWalletCertificate.load(from: fixtureURL("certificate.json"))
        let txId = IrohaHash.hash(Data("tx-id".utf8))
        let challengeHash = IrohaHash.hash(Data("challenge".utf8))
        let proof = OfflinePlatformProof.appleAppAttest(
            AppleAppAttestProof(keyId: Data("swift-tests".utf8).base64EncodedString(),
                                counter: 9,
                                assertion: Data([1, 2, 3, 4]),
                                challengeHash: challengeHash)
        )
        let snapshot = OfflinePlatformTokenSnapshot(policy: "play_integrity",
                                                     attestationJwsB64: "token")
        let signature = Data(repeating: 0xAB, count: 64)
        let issuedAtMs = certificate.issuedAtMs + 1000
        let receipt = OfflineSpendReceipt(
            txId: txId,
            from: certificate.controller,
            to: certificate.controller,
            assetId: certificate.allowance.assetId,
            amount: certificate.allowance.amount,
            issuedAtMs: issuedAtMs,
            invoiceId: "inv-001",
            platformProof: proof,
            platformSnapshot: snapshot,
            senderCertificate: certificate,
            senderSignature: signature
        )

        let json = try receipt.toriiJSON()
        guard case let .object(object) = json else {
            return XCTFail("expected receipt JSON object")
        }

        XCTAssertEqual(object["tx_id"]?.normalizedString, hashLiteral(txId))
        XCTAssertEqual(object["issued_at_ms"]?.numberValue, Double(issuedAtMs))
        XCTAssertEqual(object["sender_signature"]?.normalizedString, signature.hexUppercased())

        guard case let .object(platformProof) = object["platform_proof"] else {
            return XCTFail("expected platform_proof object")
        }
        XCTAssertEqual(platformProof["platform"]?.normalizedString, "AppleAppAttest")
        guard case let .object(proofObject) = platformProof["proof"] else {
            return XCTFail("expected platform proof payload")
        }
        XCTAssertEqual(proofObject["challenge_hash"]?.normalizedString, hashLiteral(challengeHash))

        guard case let .object(snapshotObject) = object["platform_snapshot"] else {
            return XCTFail("expected platform_snapshot object")
        }
        XCTAssertEqual(snapshotObject["policy"]?.normalizedString, snapshot.policy)
        XCTAssertEqual(snapshotObject["attestation_jws_b64"]?.normalizedString, snapshot.attestationJwsB64)

        XCTAssertEqual(
            object["sender_certificate_id"]?.normalizedString,
            hashLiteral(try certificate.certificateId())
        )
    }

    func testTransferToriiJSONEncoding() throws {
        let certificate = try OfflineWalletCertificate.load(from: fixtureURL("certificate.json"))
        let receipt = try makeReceipt(certificate: certificate)
        let resultingCommitment = Data(repeating: 0x22, count: 32)
        let balanceProof = OfflineBalanceProof(
            initialCommitment: certificate.allowance,
            resultingCommitment: resultingCommitment,
            claimedDelta: "10",
            zkProof: validZkProof()
        )
        let receiptsRoot = OfflinePoseidonDigest(bytes: Data(repeating: 0x11, count: 32))
        let aggregateProof = OfflineAggregateProofEnvelope(
            version: 1,
            receiptsRoot: receiptsRoot,
            proofSum: Data([0x10]),
            proofCounter: Data([0x11]),
            proofReplay: Data([0x12]),
            metadata: [
                OfflineAggregateProofMetadataKey.parameterSet: .string("fastpq-offline-v1"),
                OfflineAggregateProofMetadataKey.sumCircuit: .string("fastpq/offline_sum/v1"),
                OfflineAggregateProofMetadataKey.counterCircuit: .string("fastpq/offline_counter/v1"),
                OfflineAggregateProofMetadataKey.replayCircuit: .string("fastpq/offline_replay/v1"),
            ]
        )
        let attachment = try ProofAttachment(
            backend: "test",
            proof: Data([0x05, 0x06]),
            verifyingKey: .reference(.init(backend: "test", name: "vk"))
        )
        let attachments = OfflineProofAttachmentList(attachments: [attachment])
        let snapshot = OfflinePlatformTokenSnapshot(policy: "marker_key", attestationJwsB64: "snap")
        let transfer = OfflineToOnlineTransfer(
            bundleId: IrohaHash.hash(Data("bundle".utf8)),
            receiver: certificate.controller,
            depositAccount: certificate.controller,
            receipts: [receipt],
            balanceProof: balanceProof,
            aggregateProof: aggregateProof,
            attachments: attachments,
            platformSnapshot: snapshot
        )

        let json = try transfer.toriiJSON()
        guard case let .object(object) = json else {
            return XCTFail("expected transfer JSON object")
        }

        XCTAssertEqual(object["bundle_id"]?.normalizedString, hashLiteral(transfer.bundleId))
        guard case let .object(balanceObject) = object["balance_proof"] else {
            return XCTFail("expected balance_proof object")
        }
        XCTAssertEqual(balanceObject["claimed_delta"]?.normalizedString, "10")
        guard let commitmentBytes = bytes(from: balanceObject["resulting_commitment"]) else {
            return XCTFail("expected resulting_commitment byte array")
        }
        XCTAssertEqual(commitmentBytes, [UInt8](resultingCommitment))

        guard case let .object(aggregateObject) = object["aggregate_proof"] else {
            return XCTFail("expected aggregate_proof object")
        }
        guard case let .object(rootObject) = aggregateObject["receipts_root"] else {
            return XCTFail("expected receipts_root object")
        }
        XCTAssertEqual(rootObject["bytes"]?.normalizedString, receiptsRoot.bytes.hexUppercased())

        let expectedAttachments = try attachments.noritoEncoded().base64EncodedString()
        XCTAssertEqual(object["attachments"]?.normalizedString, expectedAttachments)

        guard case let .object(snapshotObject) = object["platform_snapshot"] else {
            return XCTFail("expected platform_snapshot object")
        }
        XCTAssertEqual(snapshotObject["policy"]?.normalizedString, snapshot.policy)
    }

    func testPlatformProofVariantsToriiJSONEncoding() throws {
        let markerPublicKey = Data([0x04] + Array(repeating: 0x01, count: 64))
        let markerProof = AndroidMarkerKeyProof(
            series: "series-1",
            counter: 12,
            markerPublicKey: markerPublicKey,
            markerSignature: Data(repeating: 0x02, count: 64),
            attestation: Data([0x01, 0x02])
        )
        let markerValue = try OfflinePlatformProof.androidMarkerKey(markerProof).toriiJSON()
        guard case let .object(markerObject) = markerValue else {
            return XCTFail("expected marker-key platform object")
        }
        XCTAssertEqual(markerObject["platform"]?.normalizedString, "AndroidMarkerKey")
        guard case let .object(markerPayload) = markerObject["proof"] else {
            return XCTFail("expected marker-key proof payload")
        }
        guard let markerKeyBytes = bytes(from: markerPayload["marker_public_key"]) else {
            return XCTFail("expected marker_public_key bytes")
        }
        XCTAssertEqual(markerKeyBytes, [UInt8](markerPublicKey))
        guard let signatureBytes = bytes(from: markerPayload["marker_signature"]) else {
            return XCTFail("expected marker_signature bytes")
        }
        XCTAssertEqual(signatureBytes, [UInt8](markerProof.markerSignature ?? Data()))
        guard let attestationBytes = bytes(from: markerPayload["attestation"]) else {
            return XCTFail("expected marker-key attestation bytes")
        }
        XCTAssertEqual(attestationBytes, [0x01, 0x02])

        let hash = IrohaHash.hash(Data("provisioned".utf8))
        let provisioned = try AndroidProvisionedProof(
            manifestSchema: "offline_provisioning_v1",
            manifestVersion: 1,
            manifestIssuedAtMs: 123,
            challengeHashLiteral: hashLiteral(hash),
            counter: 3,
            deviceManifest: ["android.provisioned.device_id": .string("device-1")],
            inspectorSignatureHex: Data(repeating: 0x10, count: 64).hexUppercased()
        )
        let provisionedValue = try OfflinePlatformProof.provisioned(provisioned).toriiJSON()
        guard case let .object(provisionedObject) = provisionedValue else {
            return XCTFail("expected provisioned platform object")
        }
        XCTAssertEqual(provisionedObject["platform"]?.normalizedString, "Provisioned")
        guard case let .object(provisionedPayload) = provisionedObject["proof"] else {
            return XCTFail("expected provisioned proof payload")
        }
        XCTAssertEqual(provisionedPayload["challenge_hash"]?.normalizedString, hashLiteral(hash))
    }

    func testSubmitRequestsFromModels() throws {
        let certificate = try OfflineWalletCertificate.load(from: fixtureURL("certificate.json"))
        let receipt = try makeReceipt(certificate: certificate)
        let balanceProof = OfflineBalanceProof(
            initialCommitment: certificate.allowance,
            resultingCommitment: Data(repeating: 0x22, count: 32),
            claimedDelta: "10",
            zkProof: validZkProof()
        )
        let transfer = OfflineToOnlineTransfer(
            bundleId: IrohaHash.hash(Data("bundle".utf8)),
            receiver: certificate.controller,
            depositAccount: certificate.controller,
            receipts: [receipt],
            balanceProof: balanceProof
        )

        let receiptsRequest = try ToriiOfflineSpendReceiptsSubmitRequest(receipts: [receipt])
        XCTAssertEqual(receiptsRequest.receipts.count, 1)

        let settlementRequest = try ToriiOfflineSettlementSubmitRequest(
            authority: "alice@wonderland",
            privateKey: "ed0120deadbeef",
            transfer: transfer
        )
        guard case let .object(object) = settlementRequest.transfer else {
            return XCTFail("expected transfer payload")
        }
        XCTAssertEqual(object["bundle_id"]?.normalizedString, hashLiteral(transfer.bundleId))

        let registerRequest = try ToriiOfflineAllowanceRegisterRequest(
            authority: "alice@wonderland",
            privateKey: "ed0120deadbeef",
            certificate: certificate
        )
        guard case let .object(certificateObject) = registerRequest.certificate else {
            return XCTFail("expected allowance certificate payload")
        }
        XCTAssertNotNil(certificateObject["operator_signature"]?.normalizedString)
    }

    // MARK: - Helpers

    private func fixtureURL(_ name: String) -> URL {
        var url = URL(fileURLWithPath: #filePath)
        for _ in 0..<4 {
            url.deleteLastPathComponent()
        }
        return url.appendingPathComponent("fixtures/offline_allowance/ios-demo/\(name)")
    }

    private func makeReceipt(certificate: OfflineWalletCertificate) throws -> OfflineSpendReceipt {
        let txId = IrohaHash.hash(Data("receipt".utf8))
        let challengeHash = IrohaHash.hash(Data("challenge".utf8))
        let proof = OfflinePlatformProof.appleAppAttest(
            AppleAppAttestProof(keyId: Data("swift-tests".utf8).base64EncodedString(),
                                counter: 1,
                                assertion: Data([0xAA]),
                                challengeHash: challengeHash)
        )
        return OfflineSpendReceipt(
            txId: txId,
            from: certificate.controller,
            to: certificate.controller,
            assetId: certificate.allowance.assetId,
            amount: "10",
            issuedAtMs: 1_700_000_000_000,
            invoiceId: "inv-1",
            platformProof: proof,
            platformSnapshot: nil,
            senderCertificate: certificate,
            senderSignature: Data(repeating: 0x01, count: 64)
        )
    }

    private func validZkProof(version: UInt8 = 1) -> Data {
        var proof = Data(repeating: 0, count: OfflineBalanceProofBuilder.proofLength)
        proof[0] = version
        return proof
    }

    private func hashLiteral(_ bytes: Data) -> String {
        let body = bytes.hexUppercased()
        let checksum = crc16(tag: "hash", body: body)
        return "hash:\(body)#\(String(format: "%04X", checksum))"
    }

    private func crc16(tag: String, body: String) -> UInt16 {
        var crc: UInt16 = 0xFFFF
        for byte in tag.utf8 {
            crc = updateCrc(crc, value: byte)
        }
        crc = updateCrc(crc, value: Character(":").asciiValue ?? 0)
        for byte in body.utf8 {
            crc = updateCrc(crc, value: byte)
        }
        return crc
    }

    private func updateCrc(_ crc: UInt16, value: UInt8) -> UInt16 {
        var current = crc ^ UInt16(value) << 8
        for _ in 0..<8 {
            if current & 0x8000 != 0 {
                current = (current << 1) ^ 0x1021
            } else {
                current <<= 1
            }
        }
        return current & 0xFFFF
    }

    private func bytes(from value: ToriiJSONValue?) -> [UInt8]? {
        guard case let .array(values) = value else {
            return nil
        }
        var out: [UInt8] = []
        out.reserveCapacity(values.count)
        for value in values {
            guard case let .number(number) = value,
                  number.isFinite,
                  number >= 0,
                  number <= 255,
                  number.rounded(.towardZero) == number else {
                return nil
            }
            out.append(UInt8(number))
        }
        return out
    }
}
