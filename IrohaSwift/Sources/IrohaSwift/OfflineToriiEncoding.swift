import Foundation

private enum OfflineToriiEncoding {
    static func bytesArray(_ bytes: Data) -> ToriiJSONValue {
        .array(bytes.map { .number(Double($0)) })
    }

    static func optionalBytesArray(_ bytes: Data?) -> ToriiJSONValue {
        guard let bytes else { return .null }
        return bytesArray(bytes)
    }

    static func numericString(_ value: String) throws -> ToriiJSONValue {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        _ = try OfflineNorito.encodeNumeric(trimmed)
        return .string(trimmed)
    }

    static func metadataObject(_ metadata: [String: ToriiJSONValue]) throws -> ToriiJSONValue {
        _ = try OfflineNorito.encodeMetadata(metadata)
        return .object(metadata)
    }

    static func hashLiteralValue(_ bytes: Data) throws -> ToriiJSONValue {
        .string(try hashLiteralString(bytes))
    }

    static func optionalHashLiteralValue(_ bytes: Data?) throws -> ToriiJSONValue {
        guard let bytes else { return .null }
        return try hashLiteralValue(bytes)
    }

    static func hashLiteralString(_ bytes: Data) throws -> String {
        _ = try OfflineNorito.encodeHash(bytes)
        let body = bytes.hexUppercased()
        let checksum = crc16(tag: "hash", body: body)
        return "hash:\(body)#\(String(format: "%04X", checksum))"
    }

    private static func crc16(tag: String, body: String) -> UInt16 {
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

    private static func updateCrc(_ crc: UInt16, value: UInt8) -> UInt16 {
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
}

extension OfflineAllowanceCommitment {
    func toriiJSON() throws -> ToriiJSONValue {
        let asset = assetId.trimmingCharacters(in: .whitespacesAndNewlines)
        _ = try OfflineNorito.encodeAssetId(asset)
        return .object([
            "asset": .string(asset),
            "amount": try OfflineToriiEncoding.numericString(amount),
            "commitment": OfflineToriiEncoding.bytesArray(commitment),
        ])
    }
}

extension OfflineWalletPolicy {
    func toriiJSON() throws -> ToriiJSONValue {
        .object([
            "max_balance": try OfflineToriiEncoding.numericString(maxBalance),
            "max_tx_value": try OfflineToriiEncoding.numericString(maxTxValue),
            "expires_at_ms": .number(Double(expiresAtMs)),
        ])
    }
}

extension OfflineWalletCertificateDraft {
    func toriiJSON() throws -> ToriiJSONValue {
        .object([
            "controller": .string(controller),
            "allowance": try allowance.toriiJSON(),
            "spend_public_key": .string(spendPublicKey),
            "attestation_report": OfflineToriiEncoding.bytesArray(attestationReport),
            "issued_at_ms": .number(Double(issuedAtMs)),
            "expires_at_ms": .number(Double(expiresAtMs)),
            "policy": try policy.toriiJSON(),
            "metadata": try OfflineToriiEncoding.metadataObject(metadata),
            "verdict_id": try OfflineToriiEncoding.optionalHashLiteralValue(verdictId),
            "attestation_nonce": try OfflineToriiEncoding.optionalHashLiteralValue(attestationNonce),
            "refresh_at_ms": refreshAtMs.map { .number(Double($0)) } ?? .null,
        ])
    }
}

extension OfflineWalletCertificate {
    func toriiJSON() throws -> ToriiJSONValue {
        .object([
            "controller": .string(controller),
            "operator": .string(operatorId),
            "allowance": try allowance.toriiJSON(),
            "spend_public_key": .string(spendPublicKey),
            "attestation_report": OfflineToriiEncoding.bytesArray(attestationReport),
            "issued_at_ms": .number(Double(issuedAtMs)),
            "expires_at_ms": .number(Double(expiresAtMs)),
            "policy": try policy.toriiJSON(),
            "operator_signature": .string(operatorSignature.hexUppercased()),
            "metadata": try OfflineToriiEncoding.metadataObject(metadata),
            "verdict_id": try OfflineToriiEncoding.optionalHashLiteralValue(verdictId),
            "attestation_nonce": try OfflineToriiEncoding.optionalHashLiteralValue(attestationNonce),
            "refresh_at_ms": refreshAtMs.map { .number(Double($0)) } ?? .null,
        ])
    }
}

extension OfflinePlatformTokenSnapshot {
    func toriiJSON() -> ToriiJSONValue {
        .object([
            "policy": .string(policy),
            "attestation_jws_b64": .string(attestationJwsB64),
        ])
    }
}

extension AppleAppAttestProof {
    func toriiJSON() throws -> ToriiJSONValue {
        .object([
            "key_id": .string(keyId),
            "counter": .number(Double(counter)),
            "assertion": OfflineToriiEncoding.bytesArray(assertion),
            "challenge_hash": try OfflineToriiEncoding.hashLiteralValue(challengeHash),
        ])
    }
}

extension AndroidMarkerKeyProof {
    func toriiJSON() throws -> ToriiJSONValue {
        .object([
            "series": .string(series),
            "counter": .number(Double(counter)),
            "marker_public_key": OfflineToriiEncoding.bytesArray(markerPublicKey),
            "marker_signature": OfflineToriiEncoding.optionalBytesArray(markerSignature),
            "attestation": OfflineToriiEncoding.bytesArray(attestation),
        ])
    }
}

extension AndroidProvisionedProof {
    func toriiJSON() throws -> ToriiJSONValue {
        let versionValue: ToriiJSONValue
        if let manifestVersion {
            guard manifestVersion >= 0, manifestVersion <= Int(UInt32.max) else {
                throw OfflineNoritoError.invalidLength("manifest_version")
            }
            versionValue = .number(Double(manifestVersion))
        } else {
            versionValue = .null
        }
        return .object([
            "manifest_schema": .string(manifestSchema),
            "manifest_version": versionValue,
            "manifest_issued_at_ms": .number(Double(manifestIssuedAtMs)),
            "challenge_hash": .string(challengeHashLiteral),
            "counter": .number(Double(counter)),
            "device_manifest": try OfflineToriiEncoding.metadataObject(deviceManifest),
            "inspector_signature": .string(inspectorSignatureHex),
        ])
    }
}

extension OfflinePlatformProof {
    func toriiJSON() throws -> ToriiJSONValue {
        switch self {
        case .appleAppAttest(let proof):
            return .object([
                "platform": .string("AppleAppAttest"),
                "proof": try proof.toriiJSON(),
            ])
        case .androidMarkerKey(let proof):
            return .object([
                "platform": .string("AndroidMarkerKey"),
                "proof": try proof.toriiJSON(),
            ])
        case .provisioned(let proof):
            return .object([
                "platform": .string("Provisioned"),
                "proof": try proof.toriiJSON(),
            ])
        }
    }
}

extension OfflineSpendReceipt {
    func toriiJSON() throws -> ToriiJSONValue {
        let asset = assetId.trimmingCharacters(in: .whitespacesAndNewlines)
        _ = try OfflineNorito.encodeAssetId(asset)
        return .object([
            "tx_id": try OfflineToriiEncoding.hashLiteralValue(txId),
            "from": .string(from),
            "to": .string(to),
            "asset": .string(asset),
            "amount": try OfflineToriiEncoding.numericString(amount),
            "issued_at_ms": .number(Double(issuedAtMs)),
            "invoice_id": .string(invoiceId),
            "platform_proof": try platformProof.toriiJSON(),
            "platform_snapshot": platformSnapshot.map { $0.toriiJSON() } ?? .null,
            "sender_certificate_id": try OfflineToriiEncoding.hashLiteralValue(senderCertificateId),
            "sender_signature": .string(senderSignature.hexUppercased()),
        ])
    }
}

extension OfflineBalanceProof {
    func toriiJSON() throws -> ToriiJSONValue {
        .object([
            "initial_commitment": try initialCommitment.toriiJSON(),
            "resulting_commitment": OfflineToriiEncoding.bytesArray(resultingCommitment),
            "claimed_delta": try OfflineToriiEncoding.numericString(claimedDelta),
            "zk_proof": OfflineToriiEncoding.optionalBytesArray(zkProof),
        ])
    }
}

extension OfflineCertificateBalanceProof {
    func toriiJSON() throws -> ToriiJSONValue {
        .object([
            "sender_certificate_id": try OfflineToriiEncoding.hashLiteralValue(senderCertificateId),
            "balance_proof": try balanceProof.toriiJSON(),
        ])
    }
}

extension OfflinePoseidonDigest {
    func toriiJSON() throws -> ToriiJSONValue {
        _ = try OfflineNorito.encodePoseidonDigest(bytes)
        return .object([
            "bytes": .string(bytes.hexUppercased()),
        ])
    }
}

extension OfflineAggregateProofEnvelope {
    func toriiJSON() throws -> ToriiJSONValue {
        .object([
            "version": .number(Double(version)),
            "receipts_root": try receiptsRoot.toriiJSON(),
            "proof_sum": OfflineToriiEncoding.optionalBytesArray(proofSum),
            "proof_counter": OfflineToriiEncoding.optionalBytesArray(proofCounter),
            "proof_replay": OfflineToriiEncoding.optionalBytesArray(proofReplay),
            "metadata": try OfflineToriiEncoding.metadataObject(metadata),
        ])
    }
}

extension OfflineProofAttachmentList {
    func toriiJSON() throws -> ToriiJSONValue {
        let encoded = try noritoEncoded()
        return .string(encoded.base64EncodedString())
    }
}

extension OfflineToOnlineTransfer {
    func toriiJSON() throws -> ToriiJSONValue {
        let aggregateValue: ToriiJSONValue
        if let aggregateProof {
            aggregateValue = try aggregateProof.toriiJSON()
        } else {
            aggregateValue = .null
        }
        return .object([
            "bundle_id": try OfflineToriiEncoding.hashLiteralValue(bundleId),
            "receiver": .string(receiver),
            "deposit_account": .string(depositAccount),
            "receipts": .array(try receipts.map { try $0.toriiJSON() }),
            "balance_proof": try balanceProof.toriiJSON(),
            "balance_proofs": .array(try (balanceProofs ?? []).map { try $0.toriiJSON() }),
            "aggregate_proof": aggregateValue,
            "attachments": try attachments.map { try $0.toriiJSON() } ?? .null,
            "platform_snapshot": platformSnapshot.map { $0.toriiJSON() } ?? .null,
        ])
    }
}
