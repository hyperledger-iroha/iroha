import Foundation

// Canonical objects of the attested-app suite. Field order is normative and matches the Rust
// `iroha_kagemusha_attested` crate; every ID and digest is computed over the unsigned body so
// ECDSA malleability cannot change an identity.

/// Size bounds shared by every SDK.
enum KagemushaAttestedLimits {
    static let maximumIssuerKeys = 16
    static let maximumTiers = 16
    static let maximumSigners = 8
    static let maximumCrlEntries = 65_536
    static let maximumDeltaEntries = 8
    static let maximumDeliveries = 256
    static let maximumRedemptions = 256
    static let maximumStringBytes = 1_024
    static let maximumDescriptorBytes = 16 * 1024
    static let maximumCrlBytes = 4 * 1024 * 1024
    static let maximumReceiptBytes = 6 * 1024 * 1024
    static let maximumPeerMessageBytes = 2_048
    static let zeroDigest = Data(count: 32)
}

/// One issuer signing key listed in the scheme descriptor.
struct KagemushaAttestedIssuerKeyV1: Equatable, Sendable {
    var index: UInt8
    var publicKey: Data
    var notBeforeMs: UInt64
    var notAfterMs: UInt64

    func encodePayload() -> Data {
        var writer = KagemushaAttestedWriter()
        writer.u8(index)
        writer.array(publicKey, 65)
        writer.u64(notBeforeMs)
        writer.u64(notAfterMs)
        return writer.data
    }

    static func decodePayload(_ payload: Data) throws -> Self {
        var reader = KagemushaAttestedReader(payload)
        let value = try Self(
            index: reader.u8("issuer_key.index"),
            publicKey: reader.array(65, "issuer_key.pk"),
            notBeforeMs: reader.u64("issuer_key.not_before_ms"),
            notAfterMs: reader.u64("issuer_key.not_after_ms"))
        try reader.finish()
        return value
    }
}

/// Certificate limits for one admission tier.
struct KagemushaAttestedTierV1: Equatable, Sendable {
    var tier: UInt8
    var maxBalance: UInt64
    var maxPayment: UInt64
    var maxUnsyncedOut: UInt64
    var leaseMs: UInt64

    func encodePayload() -> Data {
        var writer = KagemushaAttestedWriter()
        writer.u8(tier)
        writer.u64(maxBalance)
        writer.u64(maxPayment)
        writer.u64(maxUnsyncedOut)
        writer.u64(leaseMs)
        return writer.data
    }

    static func decodePayload(_ payload: Data) throws -> Self {
        var reader = KagemushaAttestedReader(payload)
        let value = try Self(
            tier: reader.u8("tier.tier"),
            maxBalance: reader.u64("tier.max_balance"),
            maxPayment: reader.u64("tier.max_payment"),
            maxUnsyncedOut: reader.u64("tier.max_unsynced_out"),
            leaseMs: reader.u64("tier.lease_ms"))
        try reader.finish()
        return value
    }
}

/// Per-account caps enforced by the issuer.
struct KagemushaAttestedAccountLimitsV1: Equatable, Sendable {
    var devices: UInt8
    var dailyLoad: UInt64
    var dailyUnload: UInt64

    func encodePayload() -> Data {
        var writer = KagemushaAttestedWriter()
        writer.u8(devices)
        writer.u64(dailyLoad)
        writer.u64(dailyUnload)
        return writer.data
    }

    static func decodePayload(_ payload: Data) throws -> Self {
        var reader = KagemushaAttestedReader(payload)
        let value = try Self(
            devices: reader.u8("account_limits.devices"),
            dailyLoad: reader.u64("account_limits.daily_load"),
            dailyUnload: reader.u64("account_limits.daily_unload"))
        try reader.finish()
        return value
    }
}

/// Root-signed scheme descriptor. Platform admission policy (Android, Apple, vendor roots) is
/// enforced by the issuer; devices carry those fields opaquely and verify only the root
/// signature, the scheme identity, issuer keys, tiers and peer parameters.
struct KagemushaAttestedSchemeDescriptorV1: KagemushaAttestedNoritoValue, Equatable {
    static let schemaType = "KagemushaAttestedSchemeDescriptorV1"
    static let maximumCanonicalBytes = KagemushaAttestedLimits.maximumDescriptorBytes

    var version: UInt16
    var schemeId: Data
    var descriptorEpoch: UInt64
    var chainId: String
    var assetDefinitionId: String
    var assetScale: UInt8
    var reserveAccountId: String
    var issuerURL: String
    var issuerKeys: [KagemushaAttestedIssuerKeyV1]
    /// Opaque bare payload of the Android admission policy.
    var androidPolicy: Data
    /// Opaque bare payload of the Apple admission policy.
    var applePolicy: Data
    var tiers: [KagemushaAttestedTierV1]
    var accountLimits: KagemushaAttestedAccountLimitsV1
    var receiverGraceMs: UInt64
    var headroomQuantum: UInt64
    var allowTestDevices: Bool
    var rootSignature: Data

    func unsignedPayload() -> Data {
        var writer = KagemushaAttestedWriter()
        writer.u16(version)
        writer.array(schemeId, 32)
        writer.u64(descriptorEpoch)
        writer.string(chainId)
        writer.string(assetDefinitionId)
        writer.u8(assetScale)
        writer.string(reserveAccountId)
        writer.string(issuerURL)
        writer.sequence(issuerKeys.map { $0.encodePayload() })
        writer.nested(androidPolicy)
        writer.nested(applePolicy)
        writer.sequence(tiers.map { $0.encodePayload() })
        writer.nested(accountLimits.encodePayload())
        writer.u64(receiverGraceMs)
        writer.u64(headroomQuantum)
        writer.bool(allowTestDevices)
        return writer.data
    }

    func encodePayload() -> Data {
        var writer = KagemushaAttestedWriter()
        writer.raw(unsignedPayload())
        writer.array(rootSignature, 64)
        return writer.data
    }

    static func decodePayload(_ payload: Data) throws -> Self {
        var reader = KagemushaAttestedReader(payload)
        let value = try Self(
            version: reader.u16("descriptor.version"),
            schemeId: reader.array(32, "descriptor.scheme_id"),
            descriptorEpoch: reader.u64("descriptor.descriptor_epoch"),
            chainId: reader.string("descriptor.chain_id"),
            assetDefinitionId: reader.string("descriptor.asset_definition_id"),
            assetScale: reader.u8("descriptor.asset_scale"),
            reserveAccountId: reader.string("descriptor.reserve_account_id"),
            issuerURL: reader.string("descriptor.issuer_url"),
            issuerKeys: reader.sequence(
                "descriptor.issuer_keys", maximumCount: KagemushaAttestedLimits.maximumIssuerKeys,
                KagemushaAttestedIssuerKeyV1.decodePayload),
            androidPolicy: reader.field(),
            applePolicy: reader.field(),
            tiers: reader.sequence(
                "descriptor.tiers", maximumCount: KagemushaAttestedLimits.maximumTiers,
                KagemushaAttestedTierV1.decodePayload),
            accountLimits: KagemushaAttestedAccountLimitsV1.decodePayload(reader.field()),
            receiverGraceMs: reader.u64("descriptor.receiver_grace_ms"),
            headroomQuantum: reader.u64("descriptor.headroom_quantum"),
            allowTestDevices: reader.bool("descriptor.allow_test_devices"),
            rootSignature: reader.array(64, "descriptor.root_signature"))
        try reader.finish()
        return value
    }

    /// Exact message signed by the scheme root key.
    var signingMessage: Data {
        KagemushaAttestedDomain.descriptor.bytes
            + KagemushaAttestedFraming.frame(type: Self.schemaType, payload: unsignedPayload())
    }

    /// `H(D_descriptor || canonical(unsigned descriptor))`.
    var digest: Data { KagemushaAttestedCrypto.sha256(signingMessage) }

    func issuerKey(index: UInt8) -> KagemushaAttestedIssuerKeyV1? {
        issuerKeys.first { $0.index == index }
    }

    func tier(_ tier: UInt8) -> KagemushaAttestedTierV1? {
        tiers.first { $0.tier == tier }
    }

    /// `H(D_scheme-id || chain_id || asset_definition_id || reserve_account_id || root_pk)` with
    /// each string as a canonical length-prefixed Norito string.
    static func schemeIdentity(
        chainId: String, assetDefinitionId: String, reserveAccountId: String, rootPublicKey: Data
    ) -> Data {
        KagemushaAttestedCrypto.domainHash(
            .schemeId,
            KagemushaAttestedWriter.stringValue(chainId),
            KagemushaAttestedWriter.stringValue(assetDefinitionId),
            KagemushaAttestedWriter.stringValue(reserveAccountId),
            rootPublicKey)
    }
}

/// Issuer-certified device certificate. It carries no account.
struct KagemushaAttestedDeviceCertV1: KagemushaAttestedNoritoValue, Equatable {
    static let schemaType = "KagemushaAttestedDeviceCertV1"
    static let maximumCanonicalBytes = 512

    var version: UInt16
    var schemeId: Data
    var deviceId: Data
    var devicePublicKey: Data
    var platform: UInt8
    var tier: UInt8
    var certSerial: UInt32
    var notBeforeMs: UInt64
    var notAfterMs: UInt64
    var maxBalance: UInt64
    var maxPayment: UInt64
    var maxUnsyncedOut: UInt64
    var issuerKeyIndex: UInt8
    var issuerSignature: Data

    func unsignedPayload() -> Data {
        var writer = KagemushaAttestedWriter()
        writer.u16(version)
        writer.array(schemeId, 32)
        writer.array(deviceId, 32)
        writer.array(devicePublicKey, 65)
        writer.u8(platform)
        writer.u8(tier)
        writer.u32(certSerial)
        writer.u64(notBeforeMs)
        writer.u64(notAfterMs)
        writer.u64(maxBalance)
        writer.u64(maxPayment)
        writer.u64(maxUnsyncedOut)
        writer.u8(issuerKeyIndex)
        return writer.data
    }

    func encodePayload() -> Data {
        var writer = KagemushaAttestedWriter()
        writer.raw(unsignedPayload())
        writer.array(issuerSignature, 64)
        return writer.data
    }

    static func decodePayload(_ payload: Data) throws -> Self {
        var reader = KagemushaAttestedReader(payload)
        let value = try Self(
            version: reader.u16("cert.version"),
            schemeId: reader.array(32, "cert.scheme_id"),
            deviceId: reader.array(32, "cert.device_id"),
            devicePublicKey: reader.array(65, "cert.device_pk"),
            platform: reader.u8("cert.platform"),
            tier: reader.u8("cert.tier"),
            certSerial: reader.u32("cert.cert_serial"),
            notBeforeMs: reader.u64("cert.not_before_ms"),
            notAfterMs: reader.u64("cert.not_after_ms"),
            maxBalance: reader.u64("cert.max_balance"),
            maxPayment: reader.u64("cert.max_payment"),
            maxUnsyncedOut: reader.u64("cert.max_unsynced_out"),
            issuerKeyIndex: reader.u8("cert.issuer_key_index"),
            issuerSignature: reader.array(64, "cert.issuer_signature"))
        try reader.finish()
        return value
    }

    var signingMessage: Data {
        KagemushaAttestedDomain.cert.bytes
            + KagemushaAttestedFraming.frame(type: Self.schemaType, payload: unsignedPayload())
    }

    /// `H(D_cert || canonical(unsigned cert))`; the Bootstrap subject.
    var digest: Data { KagemushaAttestedCrypto.sha256(signingMessage) }

    /// `H(D_device-id || scheme_id || device_pk)`.
    static func deviceIdentity(schemeId: Data, devicePublicKey: Data) -> Data {
        KagemushaAttestedCrypto.domainHash(.deviceId, schemeId, devicePublicKey)
    }
}

/// The six V1 relation names, reused by this suite.
enum KagemushaAttestedTransitionKind: UInt8, Sendable, CaseIterable {
    case bootstrap = 0
    case mintFold = 1
    case sendSplit = 2
    case receiveFold = 3
    case redeemSplit = 4
    case rotate = 5

    /// Outflows count toward `unsynced_out` until acknowledged by the issuer.
    var isOutflow: Bool { self == .sendSplit || self == .redeemSplit }
}

/// One signed state change of a device ledger.
struct KagemushaAttestedTransitionV1: KagemushaAttestedNoritoValue, Equatable {
    static let schemaType = "KagemushaAttestedTransitionV1"
    static let maximumCanonicalBytes = 320

    var version: UInt16
    var deviceId: Data
    var seq: UInt64
    var prevDigest: Data
    var kind: KagemushaAttestedTransitionKind
    var amount: UInt64
    var balanceAfter: UInt64
    var subject: Data
    var counterparty: Data
    var ackedSeq: UInt64
    var unsyncedOutAfter: UInt64
    var crlEpochHeld: UInt64
    var deviceTimeMs: UInt64

    func encodePayload() -> Data {
        var writer = KagemushaAttestedWriter()
        writer.u16(version)
        writer.array(deviceId, 32)
        writer.u64(seq)
        writer.array(prevDigest, 32)
        writer.u8(kind.rawValue)
        writer.u64(amount)
        writer.u64(balanceAfter)
        writer.array(subject, 32)
        writer.array(counterparty, 32)
        writer.u64(ackedSeq)
        writer.u64(unsyncedOutAfter)
        writer.u64(crlEpochHeld)
        writer.u64(deviceTimeMs)
        return writer.data
    }

    static func decodePayload(_ payload: Data) throws -> Self {
        var reader = KagemushaAttestedReader(payload)
        let version = try reader.u16("transition.version")
        let deviceId = try reader.array(32, "transition.device_id")
        let seq = try reader.u64("transition.seq")
        let prevDigest = try reader.array(32, "transition.prev_digest")
        guard let kind = KagemushaAttestedTransitionKind(rawValue: try reader.u8("transition.kind")) else {
            throw KagemushaAttestedCodecError.invalidField("transition.kind")
        }
        let value = try Self(
            version: version, deviceId: deviceId, seq: seq, prevDigest: prevDigest, kind: kind,
            amount: reader.u64("transition.amount"),
            balanceAfter: reader.u64("transition.balance_after"),
            subject: reader.array(32, "transition.subject"),
            counterparty: reader.array(32, "transition.counterparty"),
            ackedSeq: reader.u64("transition.acked_seq"),
            unsyncedOutAfter: reader.u64("transition.unsynced_out_after"),
            crlEpochHeld: reader.u64("transition.crl_epoch_held"),
            deviceTimeMs: reader.u64("transition.device_time_ms"))
        try reader.finish()
        return value
    }

    /// Exact message signed by the device key.
    var signingMessage: Data { KagemushaAttestedDomain.transition.bytes + canonicalBytes }

    /// `digest(T) = H(D_transition || T)`. A SendSplit digest is the `payment_id` nullifier.
    var digest: Data { KagemushaAttestedCrypto.sha256(signingMessage) }
}

/// A transition with its device signature, as uploaded at sync and embedded in evidence.
struct KagemushaAttestedSignedTransitionV1: Equatable, Sendable {
    var transition: KagemushaAttestedTransitionV1
    var signature: Data

    func encodePayload() -> Data {
        var writer = KagemushaAttestedWriter()
        writer.nested(transition.encodePayload())
        writer.array(signature, 64)
        return writer.data
    }

    static func decodePayload(_ payload: Data) throws -> Self {
        var reader = KagemushaAttestedReader(payload)
        let value = try Self(
            transition: KagemushaAttestedTransitionV1.decodeNested(reader.field()),
            signature: reader.array(64, "signed_transition.signature"))
        try reader.finish()
        return value
    }
}

/// One revocation entry.
struct KagemushaAttestedRevocationEntryV1: Equatable, Sendable {
    var deviceId: Data
    var reason: KagemushaRevocationReason
    var epoch: UInt64

    func encodePayload() -> Data {
        var writer = KagemushaAttestedWriter()
        writer.array(deviceId, 32)
        writer.u8(reason.rawValue)
        writer.u64(epoch)
        return writer.data
    }

    static func decodePayload(_ payload: Data) throws -> Self {
        var reader = KagemushaAttestedReader(payload)
        let deviceId = try reader.array(32, "crl_entry.device_id")
        guard let reason = KagemushaRevocationReason(rawValue: try reader.u8("crl_entry.reason")) else {
            throw KagemushaAttestedCodecError.invalidField("crl_entry.reason")
        }
        let value = try Self(deviceId: deviceId, reason: reason, epoch: reader.u64("crl_entry.epoch"))
        try reader.finish()
        return value
    }
}

/// Issuer-signed complete revocation list.
struct KagemushaAttestedRevocationListV1: KagemushaAttestedNoritoValue, Equatable {
    static let schemaType = "KagemushaAttestedRevocationListV1"
    static let maximumCanonicalBytes = KagemushaAttestedLimits.maximumCrlBytes

    var schemeId: Data
    var epoch: UInt64
    var issuedAtMs: UInt64
    var entries: [KagemushaAttestedRevocationEntryV1]
    var keyIndex: UInt8
    var signature: Data

    func unsignedPayload() -> Data {
        var writer = KagemushaAttestedWriter()
        writer.array(schemeId, 32)
        writer.u64(epoch)
        writer.u64(issuedAtMs)
        writer.sequence(entries.map { $0.encodePayload() })
        writer.u8(keyIndex)
        return writer.data
    }

    func encodePayload() -> Data {
        var writer = KagemushaAttestedWriter()
        writer.raw(unsignedPayload())
        writer.array(signature, 64)
        return writer.data
    }

    static func decodePayload(_ payload: Data) throws -> Self {
        var reader = KagemushaAttestedReader(payload)
        let value = try Self(
            schemeId: reader.array(32, "crl.scheme_id"),
            epoch: reader.u64("crl.epoch"),
            issuedAtMs: reader.u64("crl.issued_at_ms"),
            entries: reader.sequence(
                "crl.entries", maximumCount: KagemushaAttestedLimits.maximumCrlEntries,
                KagemushaAttestedRevocationEntryV1.decodePayload),
            keyIndex: reader.u8("crl.key_index"),
            signature: reader.array(64, "crl.signature"))
        try reader.finish()
        return value
    }

    var signingMessage: Data {
        KagemushaAttestedDomain.crl.bytes
            + KagemushaAttestedFraming.frame(type: Self.schemaType, payload: unsignedPayload())
    }

    func reason(for deviceId: Data) -> KagemushaRevocationReason? {
        entries.first { $0.deviceId == deviceId }?.reason
    }
}

/// Issuer-signed revocation delta gossiped offline inside Requests and Payments.
struct KagemushaAttestedRevocationDeltaV1: KagemushaAttestedNoritoValue, Equatable {
    static let schemaType = "KagemushaAttestedRevocationDeltaV1"
    static let maximumCanonicalBytes = 1_024

    var fromEpoch: UInt64
    var toEpoch: UInt64
    var entries: [KagemushaAttestedRevocationEntryV1]
    var keyIndex: UInt8
    var signature: Data

    func unsignedPayload() -> Data {
        var writer = KagemushaAttestedWriter()
        writer.u64(fromEpoch)
        writer.u64(toEpoch)
        writer.sequence(entries.map { $0.encodePayload() })
        writer.u8(keyIndex)
        return writer.data
    }

    func encodePayload() -> Data {
        var writer = KagemushaAttestedWriter()
        writer.raw(unsignedPayload())
        writer.array(signature, 64)
        return writer.data
    }

    static func decodePayload(_ payload: Data) throws -> Self {
        var reader = KagemushaAttestedReader(payload)
        let value = try Self(
            fromEpoch: reader.u64("crl_delta.from_epoch"),
            toEpoch: reader.u64("crl_delta.to_epoch"),
            entries: reader.sequence(
                "crl_delta.entries", maximumCount: KagemushaAttestedLimits.maximumDeltaEntries,
                KagemushaAttestedRevocationEntryV1.decodePayload),
            keyIndex: reader.u8("crl_delta.key_index"),
            signature: reader.array(64, "crl_delta.signature"))
        try reader.finish()
        return value
    }

    var signingMessage: Data {
        KagemushaAttestedDomain.crlDelta.bytes
            + KagemushaAttestedFraming.frame(type: Self.schemaType, payload: unsignedPayload())
    }
}

/// Receiver-signed payment request (IPM1 kind 1).
struct KagemushaAttestedPaymentRequestV1: KagemushaAttestedNoritoValue, Equatable {
    static let schemaType = "KagemushaAttestedPaymentRequestV1"
    static let maximumCanonicalBytes = KagemushaAttestedLimits.maximumPeerMessageBytes

    var version: UInt16
    var receiverCert: KagemushaAttestedDeviceCertV1
    var requestNonce: Data
    /// Zero means the payer enters the amount.
    var amount: UInt64
    var headroom: UInt64
    var reusable: Bool
    var maxUses: UInt16
    var createdAtMs: UInt64
    var crlEpochHeld: UInt64
    var crlDelta: KagemushaAttestedRevocationDeltaV1?
    var signature: Data

    func unsignedPayload() -> Data {
        var writer = KagemushaAttestedWriter()
        writer.u16(version)
        writer.nested(receiverCert.encodePayload())
        writer.array(requestNonce, 16)
        writer.u64(amount)
        writer.u64(headroom)
        writer.bool(reusable)
        writer.u16(maxUses)
        writer.u64(createdAtMs)
        writer.u64(crlEpochHeld)
        writer.option(crlDelta?.encodePayload())
        return writer.data
    }

    func encodePayload() -> Data {
        var writer = KagemushaAttestedWriter()
        writer.raw(unsignedPayload())
        writer.array(signature, 64)
        return writer.data
    }

    static func decodePayload(_ payload: Data) throws -> Self {
        var reader = KagemushaAttestedReader(payload)
        let value = try Self(
            version: reader.u16("request.version"),
            receiverCert: KagemushaAttestedDeviceCertV1.decodeNested(reader.field()),
            requestNonce: reader.array(16, "request.request_nonce"),
            amount: reader.u64("request.amount"),
            headroom: reader.u64("request.headroom"),
            reusable: reader.bool("request.reusable"),
            maxUses: reader.u16("request.max_uses"),
            createdAtMs: reader.u64("request.created_at_ms"),
            crlEpochHeld: reader.u64("request.crl_epoch_held"),
            crlDelta: reader.option("request.crl_delta", KagemushaAttestedRevocationDeltaV1.decodeNested),
            signature: reader.array(64, "request.signature"))
        try reader.finish()
        return value
    }

    var signingMessage: Data {
        KagemushaAttestedDomain.request.bytes
            + KagemushaAttestedFraming.frame(type: Self.schemaType, payload: unsignedPayload())
    }

    /// `request_digest = H(D_request || unsigned request)`; the SendSplit subject.
    var digest: Data { KagemushaAttestedCrypto.sha256(signingMessage) }
}

/// Payer's final payment (IPM1 kind 2).
struct KagemushaAttestedPaymentV1: KagemushaAttestedNoritoValue, Equatable {
    static let schemaType = "KagemushaAttestedPaymentV1"
    static let maximumCanonicalBytes = KagemushaAttestedLimits.maximumPeerMessageBytes

    var version: UInt16
    var payerCert: KagemushaAttestedDeviceCertV1
    var transition: KagemushaAttestedTransitionV1
    var signature: Data
    var crlDelta: KagemushaAttestedRevocationDeltaV1?

    func encodePayload() -> Data {
        var writer = KagemushaAttestedWriter()
        writer.u16(version)
        writer.nested(payerCert.encodePayload())
        writer.nested(transition.encodePayload())
        writer.array(signature, 64)
        writer.option(crlDelta?.encodePayload())
        return writer.data
    }

    static func decodePayload(_ payload: Data) throws -> Self {
        var reader = KagemushaAttestedReader(payload)
        let value = try Self(
            version: reader.u16("payment.version"),
            payerCert: KagemushaAttestedDeviceCertV1.decodeNested(reader.field()),
            transition: KagemushaAttestedTransitionV1.decodeNested(reader.field()),
            signature: reader.array(64, "payment.signature"),
            crlDelta: reader.option("payment.crl_delta", KagemushaAttestedRevocationDeltaV1.decodeNested))
        try reader.finish()
        return value
    }

    /// The payment identity and nullifier: the digest of the payer's SendSplit.
    var paymentId: Data { transition.digest }
}

/// Receiver's courtesy acknowledgement (IPM1 kind 3).
struct KagemushaAttestedAcknowledgementV1: KagemushaAttestedNoritoValue, Equatable {
    static let schemaType = "KagemushaAttestedAcknowledgementV1"
    static let maximumCanonicalBytes = 512

    var version: UInt16
    var paymentId: Data
    var receiverDeviceId: Data
    var receiveTransitionDigest: Data
    var signature: Data

    func unsignedPayload() -> Data {
        var writer = KagemushaAttestedWriter()
        writer.u16(version)
        writer.array(paymentId, 32)
        writer.array(receiverDeviceId, 32)
        writer.array(receiveTransitionDigest, 32)
        return writer.data
    }

    func encodePayload() -> Data {
        var writer = KagemushaAttestedWriter()
        writer.raw(unsignedPayload())
        writer.array(signature, 64)
        return writer.data
    }

    static func decodePayload(_ payload: Data) throws -> Self {
        var reader = KagemushaAttestedReader(payload)
        let value = try Self(
            version: reader.u16("ack.version"),
            paymentId: reader.array(32, "ack.payment_id"),
            receiverDeviceId: reader.array(32, "ack.receiver_device_id"),
            receiveTransitionDigest: reader.array(32, "ack.receive_transition_digest"),
            signature: reader.array(64, "ack.signature"))
        try reader.finish()
        return value
    }

    var signingMessage: Data {
        KagemushaAttestedDomain.ack.bytes
            + KagemushaAttestedFraming.frame(type: Self.schemaType, payload: unsignedPayload())
    }
}

/// Issuer-signed mint voucher for one committed load.
struct KagemushaAttestedMintVoucherV1: KagemushaAttestedNoritoValue, Equatable {
    static let schemaType = "KagemushaAttestedMintVoucherV1"
    static let maximumCanonicalBytes = 512

    var schemeId: Data
    var voucherId: Data
    var deviceId: Data
    var loadId: Data
    var amount: UInt64
    var txHash: Data
    var issuedAtMs: UInt64
    var issuerKeyIndex: UInt8
    var signature: Data

    func unsignedPayload() -> Data {
        var writer = KagemushaAttestedWriter()
        writer.array(schemeId, 32)
        writer.array(voucherId, 32)
        writer.array(deviceId, 32)
        writer.array(loadId, 16)
        writer.u64(amount)
        writer.array(txHash, 32)
        writer.u64(issuedAtMs)
        writer.u8(issuerKeyIndex)
        return writer.data
    }

    func encodePayload() -> Data {
        var writer = KagemushaAttestedWriter()
        writer.raw(unsignedPayload())
        writer.array(signature, 64)
        return writer.data
    }

    static func decodePayload(_ payload: Data) throws -> Self {
        var reader = KagemushaAttestedReader(payload)
        let value = try Self(
            schemeId: reader.array(32, "voucher.scheme_id"),
            voucherId: reader.array(32, "voucher.voucher_id"),
            deviceId: reader.array(32, "voucher.device_id"),
            loadId: reader.array(16, "voucher.load_id"),
            amount: reader.u64("voucher.amount"),
            txHash: reader.array(32, "voucher.tx_hash"),
            issuedAtMs: reader.u64("voucher.issued_at_ms"),
            issuerKeyIndex: reader.u8("voucher.issuer_key_index"),
            signature: reader.array(64, "voucher.signature"))
        try reader.finish()
        return value
    }

    var signingMessage: Data {
        KagemushaAttestedDomain.voucher.bytes
            + KagemushaAttestedFraming.frame(type: Self.schemaType, payload: unsignedPayload())
    }

    /// `voucher_id = H(D_voucher || tx_hash || load_id)`.
    static func voucherIdentity(txHash: Data, loadId: Data) -> Data {
        KagemushaAttestedCrypto.domainHash(.voucher, txHash, loadId)
    }
}

/// Issuer authorization to deliver a payment at the receiver's sync.
struct KagemushaAttestedDeliveryV1: KagemushaAttestedNoritoValue, Equatable {
    static let schemaType = "KagemushaAttestedDeliveryV1"
    static let maximumCanonicalBytes = 512

    var paymentId: Data
    var receiverDeviceId: Data
    var keyIndex: UInt8
    var signature: Data

    func unsignedPayload() -> Data {
        var writer = KagemushaAttestedWriter()
        writer.array(paymentId, 32)
        writer.array(receiverDeviceId, 32)
        writer.u8(keyIndex)
        return writer.data
    }

    func encodePayload() -> Data {
        var writer = KagemushaAttestedWriter()
        writer.raw(unsignedPayload())
        writer.array(signature, 64)
        return writer.data
    }

    static func decodePayload(_ payload: Data) throws -> Self {
        var reader = KagemushaAttestedReader(payload)
        let value = try Self(
            paymentId: reader.array(32, "delivery.payment_id"),
            receiverDeviceId: reader.array(32, "delivery.receiver_device_id"),
            keyIndex: reader.u8("delivery.key_index"),
            signature: reader.array(64, "delivery.signature"))
        try reader.finish()
        return value
    }

    var signingMessage: Data {
        KagemushaAttestedDomain.delivery.bytes
            + KagemushaAttestedFraming.frame(type: Self.schemaType, payload: unsignedPayload())
    }
}

/// Self-contained fork evidence against one device.
enum KagemushaAttestedForkEvidenceV1: KagemushaAttestedNoritoValue, Equatable {
    static let schemaType = "KagemushaAttestedForkEvidenceV1"
    static let maximumCanonicalBytes = 1_024

    /// Two distinct digests at the same `(device_id, seq)`.
    case sameSeq(
        cert: KagemushaAttestedDeviceCertV1,
        a: KagemushaAttestedSignedTransitionV1,
        b: KagemushaAttestedSignedTransitionV1)
    /// Same `acked_seq`, `seq_a < seq_b`, and a send whose `unsynced_out_after` ignores `a`.
    case inconsistent(
        cert: KagemushaAttestedDeviceCertV1,
        a: KagemushaAttestedSignedTransitionV1,
        b: KagemushaAttestedSignedTransitionV1)

    func encodePayload() -> Data {
        let (discriminant, cert, a, b): (UInt32, KagemushaAttestedDeviceCertV1,
            KagemushaAttestedSignedTransitionV1, KagemushaAttestedSignedTransitionV1)
        switch self {
        case .sameSeq(let c, let x, let y): (discriminant, cert, a, b) = (0, c, x, y)
        case .inconsistent(let c, let x, let y): (discriminant, cert, a, b) = (1, c, x, y)
        }
        return KagemushaAttestedWriter.enumValue(discriminant, [
            cert.encodePayload(),
            a.transition.encodePayload(), a.signature,
            b.transition.encodePayload(), b.signature,
        ])
    }

    static func decodePayload(_ payload: Data) throws -> Self {
        var (discriminant, reader) = try KagemushaAttestedReader.enumValue(payload, "fork_evidence")
        let cert = try KagemushaAttestedDeviceCertV1.decodeNested(reader.field())
        let a = try KagemushaAttestedSignedTransitionV1(
            transition: .decodeNested(reader.field()), signature: reader.array(64, "fork.sig_a"))
        let b = try KagemushaAttestedSignedTransitionV1(
            transition: .decodeNested(reader.field()), signature: reader.array(64, "fork.sig_b"))
        try reader.finish()
        switch discriminant {
        case 0: return .sameSeq(cert: cert, a: a, b: b)
        case 1: return .inconsistent(cert: cert, a: a, b: b)
        default: throw KagemushaAttestedCodecError.invalidField("fork_evidence.variant")
        }
    }

    var accusedDeviceId: Data {
        switch self {
        case .sameSeq(let cert, _, _), .inconsistent(let cert, _, _): return cert.deviceId
        }
    }
}

/// A payment the issuer delivers to this receiver at sync, with its authorization.
struct KagemushaAttestedReceiptDeliveryV1: Equatable, Sendable {
    var payment: KagemushaAttestedPaymentV1
    var delivery: KagemushaAttestedDeliveryV1

    func encodePayload() -> Data {
        var writer = KagemushaAttestedWriter()
        writer.nested(payment.encodePayload())
        writer.nested(delivery.encodePayload())
        return writer.data
    }

    static func decodePayload(_ payload: Data) throws -> Self {
        var reader = KagemushaAttestedReader(payload)
        let value = try Self(
            payment: KagemushaAttestedPaymentV1.decodeNested(reader.field()),
            delivery: KagemushaAttestedDeliveryV1.decodeNested(reader.field()))
        try reader.finish()
        return value
    }
}

/// Issuer-side status of one redemption.
struct KagemushaAttestedRedemptionStatusV1: Equatable, Sendable {
    enum State: UInt8, Sendable {
        case queued = 0
        case paid = 1
        case frozen = 2
    }

    var redemptionId: Data
    var state: State
    var txHash: Data?

    func encodePayload() -> Data {
        var writer = KagemushaAttestedWriter()
        writer.array(redemptionId, 32)
        writer.u8(state.rawValue)
        writer.option(txHash.map(KagemushaAttestedWriter.genericByteArrayValue))
        return writer.data
    }

    static func decodePayload(_ payload: Data) throws -> Self {
        var reader = KagemushaAttestedReader(payload)
        let redemptionId = try reader.array(32, "redemption.redemption_id")
        guard let state = State(rawValue: try reader.u8("redemption.state")) else {
            throw KagemushaAttestedCodecError.invalidField("redemption.state")
        }
        let txHash = try reader.option("redemption.tx_hash") {
            try KagemushaAttestedReader.genericByteArrayValue($0, count: 32, "redemption.tx_hash")
        }
        try reader.finish()
        return Self(redemptionId: redemptionId, state: state, txHash: txHash)
    }
}

/// Issuer-signed sync receipt.
struct KagemushaAttestedSyncReceiptV1: KagemushaAttestedNoritoValue, Equatable {
    static let schemaType = "KagemushaAttestedSyncReceiptV1"
    static let maximumCanonicalBytes = KagemushaAttestedLimits.maximumReceiptBytes

    var deviceId: Data
    var ackedSeq: UInt64
    var ackedDigest: Data
    var renewedCert: KagemushaAttestedDeviceCertV1?
    var crl: KagemushaAttestedRevocationListV1
    var deliveries: [KagemushaAttestedReceiptDeliveryV1]
    var redemptions: [KagemushaAttestedRedemptionStatusV1]
    var nextSyncNonce: Data
    var keyIndex: UInt8
    var signature: Data

    func unsignedPayload() -> Data {
        var writer = KagemushaAttestedWriter()
        writer.array(deviceId, 32)
        writer.u64(ackedSeq)
        writer.array(ackedDigest, 32)
        writer.option(renewedCert?.encodePayload())
        writer.nested(crl.encodePayload())
        writer.sequence(deliveries.map { $0.encodePayload() })
        writer.sequence(redemptions.map { $0.encodePayload() })
        writer.array(nextSyncNonce, 32)
        writer.u8(keyIndex)
        return writer.data
    }

    func encodePayload() -> Data {
        var writer = KagemushaAttestedWriter()
        writer.raw(unsignedPayload())
        writer.array(signature, 64)
        return writer.data
    }

    static func decodePayload(_ payload: Data) throws -> Self {
        var reader = KagemushaAttestedReader(payload)
        let value = try Self(
            deviceId: reader.array(32, "receipt.device_id"),
            ackedSeq: reader.u64("receipt.acked_seq"),
            ackedDigest: reader.array(32, "receipt.acked_digest"),
            renewedCert: reader.option("receipt.renewed_cert", KagemushaAttestedDeviceCertV1.decodeNested),
            crl: KagemushaAttestedRevocationListV1.decodeNested(reader.field()),
            deliveries: reader.sequence(
                "receipt.deliveries", maximumCount: KagemushaAttestedLimits.maximumDeliveries,
                KagemushaAttestedReceiptDeliveryV1.decodePayload),
            redemptions: reader.sequence(
                "receipt.redemptions", maximumCount: KagemushaAttestedLimits.maximumRedemptions,
                KagemushaAttestedRedemptionStatusV1.decodePayload),
            nextSyncNonce: reader.array(32, "receipt.next_sync_nonce"),
            keyIndex: reader.u8("receipt.key_index"),
            signature: reader.array(64, "receipt.signature"))
        try reader.finish()
        return value
    }

    var signingMessage: Data {
        KagemushaAttestedDomain.sync.bytes
            + KagemushaAttestedFraming.frame(type: Self.schemaType, payload: unsignedPayload())
    }
}

/// Transcript and derived identities used online.
enum KagemushaAttestedTranscripts {
    /// `account_digest`: SHA-256 of the canonical account identifier string.
    static func accountDigest(_ accountId: String) -> Data {
        KagemushaAttestedCrypto.sha256(KagemushaAttestedWriter.stringValue(accountId))
    }

    /// `E = D_enroll || scheme_id || server_nonce || client_nonce || account_digest || platform
    /// || attested_key_id || signing_pk`.
    static func enrollment(
        schemeId: Data, serverNonce: Data, clientNonce: Data, accountDigest: Data,
        platform: KagemushaPlatform, attestedKeyId: Data, signingPublicKey: Data
    ) -> Data {
        var out = KagemushaAttestedDomain.enroll.bytes
        out.append(schemeId)
        out.append(serverNonce)
        out.append(clientNonce)
        out.append(accountDigest)
        out.append(platform.rawValue)
        out.append(attestedKeyId)
        out.append(signingPublicKey)
        return out
    }

    /// iOS account proof message: `H(D_account-proof || E)`.
    static func appleAccountProofMessage(transcript: Data) -> Data {
        KagemushaAttestedCrypto.domainHash(.accountProof, transcript)
    }

    /// Sync attestation-refresh client data: `H(D_sync || device_id || sync_nonce || head || seq)`.
    static func syncClientData(deviceId: Data, syncNonce: Data, head: Data, seq: UInt64) -> Data {
        KagemushaAttestedCrypto.domainHash(.sync, deviceId, syncNonce, head, .kagemushaLE(seq))
    }

    /// `binding = H(D_load-binding || scheme_id || device_id || load_id || amount)`.
    static func loadBinding(schemeId: Data, deviceId: Data, loadId: Data, amount: UInt64) -> Data {
        KagemushaAttestedCrypto.domainHash(.loadBinding, schemeId, deviceId, loadId, .kagemushaLE(amount))
    }

    /// `redemption_id = H(D_redemption-id || device_id || seq || account_digest || amount)`.
    static func redemptionIdentity(deviceId: Data, seq: UInt64, accountDigest: Data, amount: UInt64) -> Data {
        KagemushaAttestedCrypto.domainHash(
            .redemptionId, deviceId, .kagemushaLE(seq), accountDigest, .kagemushaLE(amount))
    }
}
