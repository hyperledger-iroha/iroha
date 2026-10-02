import Foundation

/// A scheme descriptor verified against the application's pinned root key and scheme identity.
/// Every issuer-signed object is checked against the issuer keys this descriptor lists.
struct KagemushaAttestedScheme: Sendable {
    let descriptor: KagemushaAttestedSchemeDescriptorV1
    let canonicalDescriptor: Data
    let rootPublicKey: Data

    var schemeId: Data { descriptor.schemeId }

    /// Verify descriptor bytes: canonical encoding, root signature, pinned scheme identity and
    /// test-device policy.
    init(descriptorBytes: Data, pinnedSchemeId: Data, rootPublicKey: Data, admitsTestDescriptor: Bool) throws {
        let descriptor: KagemushaAttestedSchemeDescriptorV1
        do {
            descriptor = try KagemushaAttestedSchemeDescriptorV1.decodeCanonical(descriptorBytes)
        } catch {
            throw KagemushaError.invalidIssuerResponse("descriptor encoding")
        }
        guard descriptor.version == 1 else { throw KagemushaError.invalidIssuerResponse("descriptor version") }
        guard KagemushaAttestedCrypto.verify(
            signature: descriptor.rootSignature, message: descriptor.signingMessage, publicKey: rootPublicKey)
        else { throw KagemushaError.invalidIssuerResponse("descriptor root signature") }
        let derived = KagemushaAttestedSchemeDescriptorV1.schemeIdentity(
            chainId: descriptor.chainId, assetDefinitionId: descriptor.assetDefinitionId,
            reserveAccountId: descriptor.reserveAccountId, rootPublicKey: rootPublicKey)
        guard descriptor.schemeId == pinnedSchemeId, derived == pinnedSchemeId else {
            throw KagemushaError.invalidIssuerResponse("descriptor scheme identity")
        }
        guard descriptor.headroomQuantum > 0, !descriptor.issuerKeys.isEmpty, !descriptor.tiers.isEmpty,
              descriptor.issuerKeys.allSatisfy({ KagemushaAttestedCrypto.isValidPublicKey($0.publicKey) }),
              Set(descriptor.issuerKeys.map(\.index)).count == descriptor.issuerKeys.count
        else { throw KagemushaError.invalidIssuerResponse("descriptor parameters") }
        if descriptor.allowTestDevices && !admitsTestDescriptor {
            throw KagemushaError.unsupported(.testDescriptorRefused)
        }
        self.descriptor = descriptor
        self.canonicalDescriptor = descriptorBytes
        self.rootPublicKey = rootPublicKey
    }

    /// Whether a newer descriptor for the same scheme may replace this one.
    func admitsReplacement(_ newer: KagemushaAttestedScheme) -> Bool {
        newer.schemeId == schemeId && newer.descriptor.descriptorEpoch >= descriptor.descriptorEpoch
    }

    // MARK: Issuer signatures

    /// Verify an issuer signature. `atMs`, when known, must fall inside the key's validity window.
    func verifyIssuer(signature: Data, message: Data, keyIndex: UInt8, atMs: UInt64?) -> Bool {
        guard let key = descriptor.issuerKey(index: keyIndex) else { return false }
        if let atMs, atMs < key.notBeforeMs || atMs > key.notAfterMs { return false }
        return KagemushaAttestedCrypto.verify(signature: signature, message: message, publicKey: key.publicKey)
    }

    /// Verify a device certificate's issuer signature and internal consistency.
    func verifyCertificate(_ cert: KagemushaAttestedDeviceCertV1) -> Bool {
        cert.version == 1 && cert.schemeId == schemeId
            && KagemushaPlatform(rawValue: cert.platform) != nil
            && KagemushaAttestedCrypto.isValidPublicKey(cert.devicePublicKey)
            && cert.deviceId == KagemushaAttestedDeviceCertV1.deviceIdentity(
                schemeId: schemeId, devicePublicKey: cert.devicePublicKey)
            && cert.notBeforeMs < cert.notAfterMs
            && cert.maxPayment > 0 && cert.maxPayment <= cert.maxUnsyncedOut
            && cert.maxBalance <= KagemushaAmount.maximumMinor
            && verifyIssuer(
                signature: cert.issuerSignature, message: cert.signingMessage,
                keyIndex: cert.issuerKeyIndex, atMs: cert.notBeforeMs)
    }

    func verifyRevocationList(_ crl: KagemushaAttestedRevocationListV1) -> Bool {
        crl.schemeId == schemeId
            && crl.entries.allSatisfy { $0.epoch <= crl.epoch }
            && Set(crl.entries.map(\.deviceId)).count == crl.entries.count
            && verifyIssuer(signature: crl.signature, message: crl.signingMessage, keyIndex: crl.keyIndex, atMs: nil)
    }

    func verifyRevocationDelta(_ delta: KagemushaAttestedRevocationDeltaV1) -> Bool {
        delta.fromEpoch < delta.toEpoch
            && delta.entries.count <= KagemushaAttestedLimits.maximumDeltaEntries
            && delta.entries.allSatisfy { $0.epoch > delta.fromEpoch && $0.epoch <= delta.toEpoch }
            && verifyIssuer(signature: delta.signature, message: delta.signingMessage, keyIndex: delta.keyIndex, atMs: nil)
    }

    func verifyVoucher(_ voucher: KagemushaAttestedMintVoucherV1) -> Bool {
        voucher.schemeId == schemeId
            && voucher.voucherId == KagemushaAttestedMintVoucherV1.voucherIdentity(
                txHash: voucher.txHash, loadId: voucher.loadId)
            && voucher.amount > 0 && voucher.amount <= KagemushaAmount.maximumMinor
            && verifyIssuer(
                signature: voucher.signature, message: voucher.signingMessage,
                keyIndex: voucher.issuerKeyIndex, atMs: nil)
    }

    func verifyDelivery(_ delivery: KagemushaAttestedDeliveryV1) -> Bool {
        verifyIssuer(signature: delivery.signature, message: delivery.signingMessage, keyIndex: delivery.keyIndex, atMs: nil)
    }

    func verifyReceipt(_ receipt: KagemushaAttestedSyncReceiptV1) -> Bool {
        verifyIssuer(signature: receipt.signature, message: receipt.signingMessage, keyIndex: receipt.keyIndex, atMs: nil)
    }

    // MARK: Device signatures

    /// Verify a receiver-signed request and return its digest.
    func verifyRequest(_ request: KagemushaAttestedPaymentRequestV1) -> Bool {
        request.version == 1 && verifyCertificate(request.receiverCert)
            && request.amount <= KagemushaAmount.maximumMinor
            && (!request.reusable || request.maxUses > 0)
            && request.crlDelta.map(verifyRevocationDelta) ?? true
            && KagemushaAttestedCrypto.verify(
                signature: request.signature, message: request.signingMessage,
                publicKey: request.receiverCert.devicePublicKey)
    }

    /// Verify a signed transition under a certificate's device key.
    func verifyTransition(_ signed: KagemushaAttestedSignedTransitionV1, cert: KagemushaAttestedDeviceCertV1) -> Bool {
        signed.transition.version == 1 && signed.transition.deviceId == cert.deviceId
            && KagemushaAttestedCrypto.verify(
                signature: signed.signature, message: signed.transition.signingMessage,
                publicKey: cert.devicePublicKey)
    }

    func verifyAcknowledgement(_ ack: KagemushaAttestedAcknowledgementV1, receiverPublicKey: Data) -> Bool {
        ack.version == 1 && KagemushaAttestedCrypto.verify(
            signature: ack.signature, message: ack.signingMessage, publicKey: receiverPublicKey)
    }

    /// Self-contained verification of fork evidence against one certified device.
    func verifyForkEvidence(_ evidence: KagemushaAttestedForkEvidenceV1) -> Bool {
        switch evidence {
        case .sameSeq(let cert, let a, let b):
            return verifyCertificate(cert) && verifyTransition(a, cert: cert) && verifyTransition(b, cert: cert)
                && a.transition.seq == b.transition.seq && a.transition.digest != b.transition.digest
        case .inconsistent(let cert, let a, let b):
            return verifyCertificate(cert) && verifyTransition(a, cert: cert) && verifyTransition(b, cert: cert)
                && Self.isInconsistentPair(a.transition, b.transition)
        }
    }

    /// Same `acked_seq`, `seq_a < seq_b`, and an outflow `b` whose `unsynced_out_after` does not
    /// cover `a`'s outstanding outflows: no genuine app produces this pair.
    static func isInconsistentPair(_ a: KagemushaAttestedTransitionV1, _ b: KagemushaAttestedTransitionV1) -> Bool {
        guard a.deviceId == b.deviceId, a.ackedSeq == b.ackedSeq, a.seq < b.seq, b.kind.isOutflow else {
            return false
        }
        let (required, overflow) = a.unsyncedOutAfter.addingReportingOverflow(b.amount)
        return overflow || b.unsyncedOutAfter < required
    }
}
