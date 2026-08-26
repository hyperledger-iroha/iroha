import Foundation

fileprivate func rejectUnknownVpnFields<K: CodingKey>(
    from decoder: Decoder,
    allowedKeys: [K],
    context: String
) throws {
    let container = try decoder.container(keyedBy: ToriiAnyCodingKey.self)
    let allowed = Set(allowedKeys.map(\.stringValue))
    if let unknown = container.allKeys.first(where: { !allowed.contains($0.stringValue) }) {
        throw DecodingError.dataCorruptedError(
            forKey: unknown,
            in: container,
            debugDescription: "\(context) contains unknown field `\(unknown.stringValue)`"
        )
    }
    let present = Set(container.allKeys.map(\.stringValue))
    if let missing = allowedKeys.first(where: { !present.contains($0.stringValue) }) {
        throw DecodingError.keyNotFound(
            ToriiAnyCodingKey(missing.stringValue),
            DecodingError.Context(
                codingPath: decoder.codingPath,
                debugDescription: "\(context) is missing required field `\(missing.stringValue)`"
            )
        )
    }
}

fileprivate func decodeCanonicalVpnHex<K: CodingKey>(
    from container: KeyedDecodingContainer<K>,
    forKey key: K,
    byteCount: Int,
    field: String
) throws -> String {
    let value = try container.decode(String.self, forKey: key)
    guard value.utf8.count == byteCount * 2,
          value.utf8.allSatisfy({ byte in
              (48...57).contains(byte) || (97...102).contains(byte)
          }) else {
        throw DecodingError.dataCorruptedError(
            forKey: key,
            in: container,
            debugDescription:
                "\(field) must be exactly \(byteCount * 2) lowercase hexadecimal characters."
        )
    }
    return value
}

fileprivate func decodeCanonicalVpnTrustHex<K: CodingKey>(
    from container: KeyedDecodingContainer<K>,
    forKey key: K,
    field: String,
    allowEmpty: Bool = false,
    requireEd25519Point: Bool = false
) throws -> String {
    let value = try container.decode(String.self, forKey: key)
    if allowEmpty, value.isEmpty {
        return value
    }
    guard value.utf8.count == 64,
          value.utf8.allSatisfy({ byte in
              (48...57).contains(byte) || (97...102).contains(byte)
          }) else {
        throw DecodingError.dataCorruptedError(
            forKey: key,
            in: container,
            debugDescription: "\(field) must be exactly 64 lowercase hexadecimal characters."
        )
    }
    guard value.utf8.contains(where: { $0 != 48 }) else {
        throw DecodingError.dataCorruptedError(
            forKey: key,
            in: container,
            debugDescription: "\(field) must not be all zero."
        )
    }
    if requireEd25519Point {
        guard let bytes = Data(hexString: value),
              Ed25519PublicKeyAdmission.isValidPublicKey(bytes) else {
            throw DecodingError.dataCorruptedError(
                forKey: key,
                in: container,
                debugDescription: "\(field) must encode a canonical non-small-order Ed25519 public key."
            )
        }
    }
    return value
}

fileprivate func decodeCanonicalVpnMldsa65PublicKey<K: CodingKey>(
    from container: KeyedDecodingContainer<K>,
    forKey key: K,
    field: String,
    allowEmpty: Bool = false
) throws -> String {
    let value = try container.decode(String.self, forKey: key)
    if allowEmpty, value.isEmpty {
        return value
    }
    guard value.utf8.count == 3_904,
          value.utf8.allSatisfy({ byte in
              (48...57).contains(byte) || (97...102).contains(byte)
          }) else {
        throw DecodingError.dataCorruptedError(
            forKey: key,
            in: container,
            debugDescription:
                "\(field) must be exactly 3904 lowercase hexadecimal characters."
        )
    }
    guard value.utf8.contains(where: { $0 != 48 }) else {
        throw DecodingError.dataCorruptedError(
            forKey: key,
            in: container,
            debugDescription: "\(field) must not be the all-zero ML-DSA-65 public key."
        )
    }
    return value
}

fileprivate func decodeCanonicalVpnTlsServerName<K: CodingKey>(
    from container: KeyedDecodingContainer<K>,
    forKey key: K,
    field: String,
    allowEmpty: Bool = false
) throws -> String {
    let value = try container.decode(String.self, forKey: key)
    if allowEmpty, value.isEmpty {
        return value
    }
    let labels = value.split(separator: ".", omittingEmptySubsequences: false)
    let isAsciiLetterOrDigit: (UInt8) -> Bool = { byte in
        (48...57).contains(byte) || (97...122).contains(byte)
    }
    let valid = !value.isEmpty
        && value.utf8.count <= 253
        && labels.allSatisfy { label in
            let bytes = Array(label.utf8)
            return !bytes.isEmpty
                && bytes.count <= 63
                && isAsciiLetterOrDigit(bytes[0])
                && isAsciiLetterOrDigit(bytes[bytes.count - 1])
                && bytes.allSatisfy { isAsciiLetterOrDigit($0) || $0 == 45 }
        }
    guard valid else {
        throw DecodingError.dataCorruptedError(
            forKey: key,
            in: container,
            debugDescription: "\(field) must be a canonical lowercase ASCII DNS name."
        )
    }
    return value
}

fileprivate func decodeCanonicalVpnEvenHex<K: CodingKey>(
    from container: KeyedDecodingContainer<K>,
    forKey key: K,
    field: String
) throws -> String {
    let value = try container.decode(String.self, forKey: key)
    guard !value.isEmpty,
          value.utf8.count.isMultiple(of: 2),
          value.utf8.allSatisfy({ byte in
              (48...57).contains(byte) || (97...102).contains(byte)
          }) else {
        throw DecodingError.dataCorruptedError(
            forKey: key,
            in: container,
            debugDescription: "\(field) must be non-empty even-length lowercase hexadecimal."
        )
    }
    return value
}

fileprivate let toriiVpnExitClasses: Set<String> = [
    "standard", "low-latency", "high-security"
]

fileprivate func decodeNonEmptyVpnString<K: CodingKey>(
    from container: KeyedDecodingContainer<K>,
    forKey key: K,
    field: String
) throws -> String {
    let value = try container.decode(String.self, forKey: key)
    guard !value.isEmpty else {
        throw DecodingError.dataCorruptedError(
            forKey: key,
            in: container,
            debugDescription: "\(field) must be a non-empty string."
        )
    }
    return value
}

fileprivate func decodeVpnUInt64<K: CodingKey>(
    from container: KeyedDecodingContainer<K>,
    forKey key: K,
    field: String
) throws -> UInt64 {
    do {
        return try container.decode(UInt64.self, forKey: key)
    } catch {
        throw DecodingError.dataCorruptedError(
            forKey: key,
            in: container,
            debugDescription: "\(field) must be a JSON unsigned integer."
        )
    }
}

fileprivate func decodeVpnUInt8<K: CodingKey>(
    from container: KeyedDecodingContainer<K>,
    forKey key: K,
    field: String
) throws -> UInt8 {
    do {
        return try container.decode(UInt8.self, forKey: key)
    } catch {
        throw DecodingError.dataCorruptedError(
            forKey: key,
            in: container,
            debugDescription: "\(field) must be a JSON unsigned 8-bit integer."
        )
    }
}

fileprivate func decodeVpnUInt16<K: CodingKey>(
    from container: KeyedDecodingContainer<K>,
    forKey key: K,
    field: String
) throws -> UInt16 {
    do {
        return try container.decode(UInt16.self, forKey: key)
    } catch {
        throw DecodingError.dataCorruptedError(
            forKey: key,
            in: container,
            debugDescription: "\(field) must be a JSON unsigned 16-bit integer."
        )
    }
}

fileprivate func decodeVpnEnum<K: CodingKey>(
    from container: KeyedDecodingContainer<K>,
    forKey key: K,
    allowed: Set<String>,
    field: String
) throws -> String {
    let value = try container.decode(String.self, forKey: key)
    guard allowed.contains(value) else {
        throw DecodingError.dataCorruptedError(
            forKey: key,
            in: container,
            debugDescription: "\(field) has an unsupported value."
        )
    }
    return value
}

fileprivate func decodeVpnSupportedExitClasses<K: CodingKey>(
    from container: KeyedDecodingContainer<K>,
    forKey key: K,
    field: String
) throws -> [String] {
    let values = try container.decode([String].self, forKey: key)
    guard values.count == 3,
          Set(values).count == 3,
          values.allSatisfy(toriiVpnExitClasses.contains) else {
        throw DecodingError.dataCorruptedError(
            forKey: key,
            in: container,
            debugDescription: "\(field) must contain each supported exit class exactly once."
        )
    }
    return values
}

fileprivate func decodeBoundedVpnUInt64<K: CodingKey>(
    from container: KeyedDecodingContainer<K>,
    forKey key: K,
    range: ClosedRange<UInt64>,
    field: String
) throws -> UInt64 {
    let value = try decodeVpnUInt64(from: container, forKey: key, field: field)
    guard range.contains(value) else {
        throw DecodingError.dataCorruptedError(
            forKey: key,
            in: container,
            debugDescription: "\(field) must be in \(range.lowerBound)...\(range.upperBound)."
        )
    }
    return value
}

fileprivate func decodeExactVpnUInt64<K: CodingKey>(
    from container: KeyedDecodingContainer<K>,
    forKey key: K,
    expected: UInt64,
    field: String
) throws -> UInt64 {
    let value = try decodeVpnUInt64(from: container, forKey: key, field: field)
    guard value == expected else {
        throw DecodingError.dataCorruptedError(
            forKey: key,
            in: container,
            debugDescription: "\(field) must equal \(expected)."
        )
    }
    return value
}

fileprivate func decodeExactVpnUInt8<K: CodingKey>(
    from container: KeyedDecodingContainer<K>,
    forKey key: K,
    expected: UInt8,
    field: String
) throws -> UInt8 {
    let value = try decodeVpnUInt8(from: container, forKey: key, field: field)
    guard value == expected else {
        throw DecodingError.dataCorruptedError(
            forKey: key,
            in: container,
            debugDescription: "\(field) must equal \(expected)."
        )
    }
    return value
}

fileprivate func decodePositiveVpnUInt16<K: CodingKey>(
    from container: KeyedDecodingContainer<K>,
    forKey key: K,
    field: String
) throws -> UInt16 {
    let value = try decodeVpnUInt16(from: container, forKey: key, field: field)
    guard value > 0 else {
        throw DecodingError.dataCorruptedError(
            forKey: key,
            in: container,
            debugDescription: "\(field) must be in 1...65535."
        )
    }
    return value
}

public struct ToriiVpnProfile: Decodable, Sendable, Equatable {
    public let available: Bool
    public let supportedExitClasses: [String]
    public let defaultExitClass: String
    public let relayEndpoint: String
    public let leaseSeconds: UInt64
    public let dnsPushIntervalSecs: UInt64
    public let routePushes: [String]
    public let excludedRoutes: [String]
    public let dnsServers: [String]
    public let tunnelAddresses: [String]
    public let mtuBytes: UInt64
    public let meterFamily: String
    public let displayBillingLabel: String
    public let operatorAccountId: String
    public let leaseFee: String
    public let settlementGraceSeconds: UInt64
    public let flowLabelBits: UInt8
    public let paddingBudgetMilliseconds: UInt16
    public let relayIdHex: String
    public let relayMldsa65PublicKeyHex: String
    public let descriptorCommitHex: String
    public let tlsServerName: String
    public let relayTlsSpkiSha256Hex: String
    public let relayCertificateSha256Hex: String
    public let directorySnapshotDigestHex: String

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case available
        case supportedExitClasses = "supported_exit_classes"
        case defaultExitClass = "default_exit_class"
        case relayEndpoint = "relay_endpoint"
        case leaseSeconds = "lease_secs"
        case dnsPushIntervalSecs = "dns_push_interval_secs"
        case routePushes = "route_pushes"
        case excludedRoutes = "excluded_routes"
        case dnsServers = "dns_servers"
        case tunnelAddresses = "tunnel_addresses"
        case mtuBytes = "mtu_bytes"
        case meterFamily = "meter_family"
        case displayBillingLabel = "display_billing_label"
        case operatorAccountId = "operator_account_id"
        case leaseFee = "lease_fee"
        case settlementGraceSeconds = "settlement_grace_secs"
        case flowLabelBits = "flow_label_bits"
        case paddingBudgetMilliseconds = "padding_budget_ms"
        case relayIdHex = "relay_id_hex"
        case relayMldsa65PublicKeyHex = "relay_mldsa65_public_key_hex"
        case descriptorCommitHex = "descriptor_commit_hex"
        case tlsServerName = "tls_server_name"
        case relayTlsSpkiSha256Hex = "relay_tls_spki_sha256_hex"
        case relayCertificateSha256Hex = "relay_certificate_sha256_hex"
        case directorySnapshotDigestHex = "directory_snapshot_digest_hex"
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownVpnFields(
            from: decoder,
            allowedKeys: CodingKeys.allCases,
            context: "vpn profile response"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        available = try container.decode(Bool.self, forKey: .available)
        supportedExitClasses = try decodeVpnSupportedExitClasses(
            from: container,
            forKey: .supportedExitClasses,
            field: "vpn profile supported_exit_classes"
        )
        defaultExitClass = try decodeVpnEnum(
            from: container,
            forKey: .defaultExitClass,
            allowed: toriiVpnExitClasses,
            field: "vpn profile default_exit_class"
        )
        if available {
            relayEndpoint = try decodeNonEmptyVpnString(
                from: container,
                forKey: .relayEndpoint,
                field: "vpn profile relay_endpoint"
            )
        } else {
            relayEndpoint = try container.decode(String.self, forKey: .relayEndpoint)
        }
        leaseSeconds = try decodeBoundedVpnUInt64(
            from: container,
            forKey: .leaseSeconds,
            range: 1...UInt64(UInt32.max),
            field: "vpn profile lease_secs"
        )
        dnsPushIntervalSecs = try decodeVpnUInt64(
            from: container,
            forKey: .dnsPushIntervalSecs,
            field: "vpn profile dns_push_interval_secs"
        )
        guard dnsPushIntervalSecs >= 30 else {
            throw DecodingError.dataCorruptedError(
                forKey: .dnsPushIntervalSecs,
                in: container,
                debugDescription: "vpn profile dns_push_interval_secs must be at least 30."
            )
        }
        routePushes = try container.decode([String].self, forKey: .routePushes)
        excludedRoutes = try container.decode([String].self, forKey: .excludedRoutes)
        dnsServers = try container.decode([String].self, forKey: .dnsServers)
        tunnelAddresses = try container.decode([String].self, forKey: .tunnelAddresses)
        mtuBytes = try decodeExactVpnUInt64(
            from: container,
            forKey: .mtuBytes,
            expected: 1_280,
            field: "vpn profile mtu_bytes"
        )
        meterFamily = try decodeNonEmptyVpnString(
            from: container,
            forKey: .meterFamily,
            field: "vpn profile meter_family"
        )
        displayBillingLabel = try decodeNonEmptyVpnString(
            from: container,
            forKey: .displayBillingLabel,
            field: "vpn profile display_billing_label"
        )
        operatorAccountId = try decodeNonEmptyVpnString(
            from: container,
            forKey: .operatorAccountId,
            field: "vpn profile operator_account_id"
        )
        leaseFee = try decodeCanonicalToriiQuantity(
            container.decode(String.self, forKey: .leaseFee),
            field: "vpn profile lease_fee"
        )
        settlementGraceSeconds = try decodeBoundedVpnUInt64(
            from: container,
            forKey: .settlementGraceSeconds,
            range: 1...UInt64.max,
            field: "vpn profile settlement_grace_secs"
        )
        flowLabelBits = try decodeExactVpnUInt8(
            from: container,
            forKey: .flowLabelBits,
            expected: 24,
            field: "vpn profile flow_label_bits"
        )
        paddingBudgetMilliseconds = try decodePositiveVpnUInt16(
            from: container,
            forKey: .paddingBudgetMilliseconds,
            field: "vpn profile padding_budget_ms"
        )
        relayIdHex = try decodeCanonicalVpnTrustHex(
            from: container,
            forKey: .relayIdHex,
            field: "vpn profile relay_id_hex",
            allowEmpty: !available,
            requireEd25519Point: true
        )
        relayMldsa65PublicKeyHex = try decodeCanonicalVpnMldsa65PublicKey(
            from: container,
            forKey: .relayMldsa65PublicKeyHex,
            field: "vpn profile relay_mldsa65_public_key_hex",
            allowEmpty: !available
        )
        descriptorCommitHex = try decodeCanonicalVpnTrustHex(
            from: container,
            forKey: .descriptorCommitHex,
            field: "vpn profile descriptor_commit_hex",
            allowEmpty: !available
        )
        tlsServerName = try decodeCanonicalVpnTlsServerName(
            from: container,
            forKey: .tlsServerName,
            field: "vpn profile tls_server_name",
            allowEmpty: !available
        )
        relayTlsSpkiSha256Hex = try decodeCanonicalVpnTrustHex(
            from: container,
            forKey: .relayTlsSpkiSha256Hex,
            field: "vpn profile relay_tls_spki_sha256_hex",
            allowEmpty: !available
        )
        relayCertificateSha256Hex = try decodeCanonicalVpnTrustHex(
            from: container,
            forKey: .relayCertificateSha256Hex,
            field: "vpn profile relay_certificate_sha256_hex",
            allowEmpty: !available
        )
        directorySnapshotDigestHex = try decodeCanonicalVpnTrustHex(
            from: container,
            forKey: .directorySnapshotDigestHex,
            field: "vpn profile directory_snapshot_digest_hex",
            allowEmpty: !available
        )
    }
}

public struct ToriiVpnTxInstruction: Decodable, Sendable, Equatable {
    public let wireId: String
    public let payloadHex: String

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case wireId = "wire_id"
        case payloadHex = "payload_hex"
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownVpnFields(
            from: decoder,
            allowedKeys: CodingKeys.allCases,
            context: "vpn transaction instruction"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        wireId = try decodeNonEmptyVpnString(
            from: container,
            forKey: .wireId,
            field: "vpn transaction instruction wire_id"
        )
        payloadHex = try decodeCanonicalVpnEvenHex(
            from: container,
            forKey: .payloadHex,
            field: "vpn transaction instruction payload_hex"
        )
    }
}

public struct ToriiVpnQuoteCreateRequest: Encodable, Sendable, Equatable {
    public var exitClass: String?
    public var meteringPublicKeyHex: String

    private enum CodingKeys: String, CodingKey {
        case exitClass = "exit_class"
        case meteringPublicKeyHex = "metering_public_key_hex"
    }

    public init(exitClass: String? = nil, meteringPublicKeyHex: String) {
        self.exitClass = exitClass
        self.meteringPublicKeyHex = meteringPublicKeyHex
    }

    public func encode(to encoder: Encoder) throws {
        let normalizedExitClass = try ToriiRequestValidation.normalizedOptionalNonEmpty(exitClass,
                                                                                       field: "exit_class") ?? ""
        let normalizedMeteringKey = try ToriiRequestValidation.normalized32ByteHex(meteringPublicKeyHex,
                                                                                   field: "metering_public_key_hex")
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(normalizedExitClass, forKey: .exitClass)
        try container.encode(normalizedMeteringKey, forKey: .meteringPublicKeyHex)
    }
}

public struct ToriiVpnQuote: Decodable, Sendable, Equatable {
    public let quoteId: String
    public let leaseIdHex: String
    public let sessionIdHex: String
    public let paymentReference: String
    public let accountId: String
    public let exitClass: String
    public let relayEndpoint: String
    public let leaseSeconds: UInt64
    public let quoteExpiresAtMilliseconds: UInt64
    public let feeAssetId: String
    public let escrowAccountId: String
    public let operatorAccountId: String
    public let leaseFee: String
    public let routePushes: [String]
    public let excludedRoutes: [String]
    public let dnsServers: [String]
    public let tunnelAddresses: [String]
    public let mtuBytes: UInt64
    public let meterFamily: String
    public let flowLabelBits: UInt8
    public let paddingBudgetMilliseconds: UInt16
    public let relayIdHex: String
    public let relayMldsa65PublicKeyHex: String
    public let descriptorCommitHex: String
    public let tlsServerName: String
    public let relayTlsSpkiSha256Hex: String
    public let relayCertificateSha256Hex: String
    public let directorySnapshotDigestHex: String
    public let meteringPublicKeyHex: String
    public let openLeaseInstruction: ToriiVpnTxInstruction

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case quoteId = "quote_id"
        case leaseIdHex = "lease_id_hex"
        case sessionIdHex = "session_id_hex"
        case paymentReference = "payment_reference"
        case accountId = "account_id"
        case exitClass = "exit_class"
        case relayEndpoint = "relay_endpoint"
        case leaseSeconds = "lease_secs"
        case quoteExpiresAtMilliseconds = "quote_expires_at_ms"
        case feeAssetId = "fee_asset_id"
        case escrowAccountId = "escrow_account_id"
        case operatorAccountId = "operator_account_id"
        case leaseFee = "lease_fee"
        case routePushes = "route_pushes"
        case excludedRoutes = "excluded_routes"
        case dnsServers = "dns_servers"
        case tunnelAddresses = "tunnel_addresses"
        case mtuBytes = "mtu_bytes"
        case meterFamily = "meter_family"
        case flowLabelBits = "flow_label_bits"
        case paddingBudgetMilliseconds = "padding_budget_ms"
        case relayIdHex = "relay_id_hex"
        case relayMldsa65PublicKeyHex = "relay_mldsa65_public_key_hex"
        case descriptorCommitHex = "descriptor_commit_hex"
        case tlsServerName = "tls_server_name"
        case relayTlsSpkiSha256Hex = "relay_tls_spki_sha256_hex"
        case relayCertificateSha256Hex = "relay_certificate_sha256_hex"
        case directorySnapshotDigestHex = "directory_snapshot_digest_hex"
        case meteringPublicKeyHex = "metering_public_key_hex"
        case openLeaseInstruction = "open_lease_instruction"
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownVpnFields(
            from: decoder,
            allowedKeys: CodingKeys.allCases,
            context: "vpn quote response"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        quoteId = try decodeCanonicalVpnHex(
            from: container,
            forKey: .quoteId,
            byteCount: 32,
            field: "vpn quote quote_id"
        )
        leaseIdHex = try decodeCanonicalVpnHex(
            from: container,
            forKey: .leaseIdHex,
            byteCount: 32,
            field: "vpn quote lease_id_hex"
        )
        sessionIdHex = try decodeCanonicalVpnHex(
            from: container,
            forKey: .sessionIdHex,
            byteCount: 16,
            field: "vpn quote session_id_hex"
        )
        paymentReference = try decodeCanonicalVpnHex(
            from: container,
            forKey: .paymentReference,
            byteCount: 32,
            field: "vpn quote payment_reference"
        )
        accountId = try decodeNonEmptyVpnString(
            from: container,
            forKey: .accountId,
            field: "vpn quote account_id"
        )
        exitClass = try decodeVpnEnum(
            from: container,
            forKey: .exitClass,
            allowed: toriiVpnExitClasses,
            field: "vpn quote exit_class"
        )
        relayEndpoint = try decodeNonEmptyVpnString(
            from: container,
            forKey: .relayEndpoint,
            field: "vpn quote relay_endpoint"
        )
        leaseSeconds = try decodeBoundedVpnUInt64(
            from: container,
            forKey: .leaseSeconds,
            range: 1...UInt64(UInt32.max),
            field: "vpn quote lease_secs"
        )
        quoteExpiresAtMilliseconds = try decodeVpnUInt64(
            from: container,
            forKey: .quoteExpiresAtMilliseconds,
            field: "vpn quote quote_expires_at_ms"
        )
        feeAssetId = try decodeNonEmptyVpnString(
            from: container,
            forKey: .feeAssetId,
            field: "vpn quote fee_asset_id"
        )
        escrowAccountId = try decodeNonEmptyVpnString(
            from: container,
            forKey: .escrowAccountId,
            field: "vpn quote escrow_account_id"
        )
        operatorAccountId = try decodeNonEmptyVpnString(
            from: container,
            forKey: .operatorAccountId,
            field: "vpn quote operator_account_id"
        )
        leaseFee = try decodeCanonicalToriiQuantity(
            container.decode(String.self, forKey: .leaseFee),
            field: "vpn quote lease_fee"
        )
        routePushes = try container.decode([String].self, forKey: .routePushes)
        excludedRoutes = try container.decode([String].self, forKey: .excludedRoutes)
        dnsServers = try container.decode([String].self, forKey: .dnsServers)
        tunnelAddresses = try container.decode([String].self, forKey: .tunnelAddresses)
        mtuBytes = try decodeExactVpnUInt64(
            from: container,
            forKey: .mtuBytes,
            expected: 1_280,
            field: "vpn quote mtu_bytes"
        )
        meterFamily = try decodeNonEmptyVpnString(
            from: container,
            forKey: .meterFamily,
            field: "vpn quote meter_family"
        )
        flowLabelBits = try decodeExactVpnUInt8(
            from: container,
            forKey: .flowLabelBits,
            expected: 24,
            field: "vpn quote flow_label_bits"
        )
        paddingBudgetMilliseconds = try decodePositiveVpnUInt16(
            from: container,
            forKey: .paddingBudgetMilliseconds,
            field: "vpn quote padding_budget_ms"
        )
        relayIdHex = try decodeCanonicalVpnTrustHex(
            from: container,
            forKey: .relayIdHex,
            field: "vpn quote relay_id_hex",
            requireEd25519Point: true
        )
        relayMldsa65PublicKeyHex = try decodeCanonicalVpnMldsa65PublicKey(
            from: container,
            forKey: .relayMldsa65PublicKeyHex,
            field: "vpn quote relay_mldsa65_public_key_hex"
        )
        descriptorCommitHex = try decodeCanonicalVpnTrustHex(
            from: container,
            forKey: .descriptorCommitHex,
            field: "vpn quote descriptor_commit_hex"
        )
        tlsServerName = try decodeCanonicalVpnTlsServerName(
            from: container,
            forKey: .tlsServerName,
            field: "vpn quote tls_server_name"
        )
        relayTlsSpkiSha256Hex = try decodeCanonicalVpnTrustHex(
            from: container,
            forKey: .relayTlsSpkiSha256Hex,
            field: "vpn quote relay_tls_spki_sha256_hex"
        )
        relayCertificateSha256Hex = try decodeCanonicalVpnTrustHex(
            from: container,
            forKey: .relayCertificateSha256Hex,
            field: "vpn quote relay_certificate_sha256_hex"
        )
        directorySnapshotDigestHex = try decodeCanonicalVpnTrustHex(
            from: container,
            forKey: .directorySnapshotDigestHex,
            field: "vpn quote directory_snapshot_digest_hex"
        )
        meteringPublicKeyHex = try decodeCanonicalVpnHex(
            from: container,
            forKey: .meteringPublicKeyHex,
            byteCount: 32,
            field: "vpn quote metering_public_key_hex"
        )
        openLeaseInstruction = try container.decode(
            ToriiVpnTxInstruction.self,
            forKey: .openLeaseInstruction
        )
    }
}

public struct ToriiVpnSessionCreateRequest: Encodable, Sendable, Equatable {
    public var exitClass: String?
    public var quoteId: String
    public var paymentTransactionHash: String
    public var meteringPublicKeyHex: String

    private enum CodingKeys: String, CodingKey {
        case exitClass = "exit_class"
        case quoteId = "quote_id"
        case paymentTransactionHash = "payment_tx_hash"
        case meteringPublicKeyHex = "metering_public_key_hex"
    }

    public init(exitClass: String? = nil,
                quoteId: String,
                paymentTransactionHash: String,
                meteringPublicKeyHex: String) {
        self.exitClass = exitClass
        self.quoteId = quoteId
        self.paymentTransactionHash = paymentTransactionHash
        self.meteringPublicKeyHex = meteringPublicKeyHex
    }

    public func encode(to encoder: Encoder) throws {
        let normalizedExitClass = try ToriiRequestValidation.normalizedOptionalNonEmpty(exitClass,
                                                                                       field: "exit_class") ?? ""
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(normalizedExitClass, forKey: .exitClass)
        try container.encode(try ToriiRequestValidation.normalized32ByteHex(quoteId, field: "quote_id"),
                             forKey: .quoteId)
        try container.encode(try ToriiRequestValidation.normalized32ByteHex(paymentTransactionHash,
                                                                            field: "payment_tx_hash"),
                             forKey: .paymentTransactionHash)
        try container.encode(try ToriiRequestValidation.normalized32ByteHex(meteringPublicKeyHex,
                                                                            field: "metering_public_key_hex"),
                             forKey: .meteringPublicKeyHex)
    }
}

public struct ToriiVpnSession: Decodable, Sendable, Equatable {
    public let sessionId: String
    public let accountId: String
    public let exitClass: String
    public let relayEndpoint: String
    public let leaseSeconds: UInt64
    public let expiresAtMilliseconds: UInt64
    public let connectedAtMilliseconds: UInt64
    public let meterFamily: String
    public let quoteId: String
    public let paymentReference: String
    public let paymentTransactionHash: String
    public let feeAssetId: String
    public let escrowAccountId: String
    public let operatorAccountId: String
    public let leaseFee: String
    public let flowLabelBits: UInt8
    public let paddingBudgetMilliseconds: UInt16
    public let relayIdHex: String
    public let relayMldsa65PublicKeyHex: String
    public let descriptorCommitHex: String
    public let tlsServerName: String
    public let relayTlsSpkiSha256Hex: String
    public let relayCertificateSha256Hex: String
    public let directorySnapshotDigestHex: String
    public let routePushes: [String]
    public let excludedRoutes: [String]
    public let dnsServers: [String]
    public let tunnelAddresses: [String]
    public let mtuBytes: UInt64
    public let helperTicketHex: String
    public let bytesIn: UInt64
    public let bytesOut: UInt64
    public let status: String

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case sessionId = "session_id"
        case accountId = "account_id"
        case exitClass = "exit_class"
        case relayEndpoint = "relay_endpoint"
        case leaseSeconds = "lease_secs"
        case expiresAtMilliseconds = "expires_at_ms"
        case connectedAtMilliseconds = "connected_at_ms"
        case meterFamily = "meter_family"
        case quoteId = "quote_id"
        case paymentReference = "payment_reference"
        case paymentTransactionHash = "payment_tx_hash"
        case feeAssetId = "fee_asset_id"
        case escrowAccountId = "escrow_account_id"
        case operatorAccountId = "operator_account_id"
        case leaseFee = "lease_fee"
        case flowLabelBits = "flow_label_bits"
        case paddingBudgetMilliseconds = "padding_budget_ms"
        case relayIdHex = "relay_id_hex"
        case relayMldsa65PublicKeyHex = "relay_mldsa65_public_key_hex"
        case descriptorCommitHex = "descriptor_commit_hex"
        case tlsServerName = "tls_server_name"
        case relayTlsSpkiSha256Hex = "relay_tls_spki_sha256_hex"
        case relayCertificateSha256Hex = "relay_certificate_sha256_hex"
        case directorySnapshotDigestHex = "directory_snapshot_digest_hex"
        case routePushes = "route_pushes"
        case excludedRoutes = "excluded_routes"
        case dnsServers = "dns_servers"
        case tunnelAddresses = "tunnel_addresses"
        case mtuBytes = "mtu_bytes"
        case helperTicketHex = "helper_ticket_hex"
        case bytesIn = "bytes_in"
        case bytesOut = "bytes_out"
        case status
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownVpnFields(
            from: decoder,
            allowedKeys: CodingKeys.allCases,
            context: "vpn session response"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        sessionId = try decodeCanonicalVpnHex(
            from: container,
            forKey: .sessionId,
            byteCount: 16,
            field: "vpn session session_id"
        )
        accountId = try decodeNonEmptyVpnString(
            from: container,
            forKey: .accountId,
            field: "vpn session account_id"
        )
        exitClass = try decodeVpnEnum(
            from: container,
            forKey: .exitClass,
            allowed: toriiVpnExitClasses,
            field: "vpn session exit_class"
        )
        relayEndpoint = try decodeNonEmptyVpnString(
            from: container,
            forKey: .relayEndpoint,
            field: "vpn session relay_endpoint"
        )
        leaseSeconds = try decodeBoundedVpnUInt64(
            from: container,
            forKey: .leaseSeconds,
            range: 1...UInt64(UInt32.max),
            field: "vpn session lease_secs"
        )
        expiresAtMilliseconds = try decodeVpnUInt64(
            from: container,
            forKey: .expiresAtMilliseconds,
            field: "vpn session expires_at_ms"
        )
        connectedAtMilliseconds = try decodeVpnUInt64(
            from: container,
            forKey: .connectedAtMilliseconds,
            field: "vpn session connected_at_ms"
        )
        meterFamily = try decodeNonEmptyVpnString(
            from: container,
            forKey: .meterFamily,
            field: "vpn session meter_family"
        )
        quoteId = try decodeCanonicalVpnHex(
            from: container,
            forKey: .quoteId,
            byteCount: 32,
            field: "vpn session quote_id"
        )
        paymentReference = try decodeCanonicalVpnHex(
            from: container,
            forKey: .paymentReference,
            byteCount: 32,
            field: "vpn session payment_reference"
        )
        paymentTransactionHash = try decodeCanonicalVpnHex(
            from: container,
            forKey: .paymentTransactionHash,
            byteCount: 32,
            field: "vpn session payment_tx_hash"
        )
        feeAssetId = try decodeNonEmptyVpnString(
            from: container,
            forKey: .feeAssetId,
            field: "vpn session fee_asset_id"
        )
        escrowAccountId = try decodeNonEmptyVpnString(
            from: container,
            forKey: .escrowAccountId,
            field: "vpn session escrow_account_id"
        )
        operatorAccountId = try decodeNonEmptyVpnString(
            from: container,
            forKey: .operatorAccountId,
            field: "vpn session operator_account_id"
        )
        leaseFee = try decodeCanonicalToriiQuantity(
            container.decode(String.self, forKey: .leaseFee),
            field: "vpn session lease_fee"
        )
        flowLabelBits = try decodeExactVpnUInt8(
            from: container,
            forKey: .flowLabelBits,
            expected: 24,
            field: "vpn session flow_label_bits"
        )
        paddingBudgetMilliseconds = try decodePositiveVpnUInt16(
            from: container,
            forKey: .paddingBudgetMilliseconds,
            field: "vpn session padding_budget_ms"
        )
        relayIdHex = try decodeCanonicalVpnTrustHex(
            from: container,
            forKey: .relayIdHex,
            field: "vpn session relay_id_hex",
            requireEd25519Point: true
        )
        relayMldsa65PublicKeyHex = try decodeCanonicalVpnMldsa65PublicKey(
            from: container,
            forKey: .relayMldsa65PublicKeyHex,
            field: "vpn session relay_mldsa65_public_key_hex"
        )
        descriptorCommitHex = try decodeCanonicalVpnTrustHex(
            from: container,
            forKey: .descriptorCommitHex,
            field: "vpn session descriptor_commit_hex"
        )
        tlsServerName = try decodeCanonicalVpnTlsServerName(
            from: container,
            forKey: .tlsServerName,
            field: "vpn session tls_server_name"
        )
        relayTlsSpkiSha256Hex = try decodeCanonicalVpnTrustHex(
            from: container,
            forKey: .relayTlsSpkiSha256Hex,
            field: "vpn session relay_tls_spki_sha256_hex"
        )
        relayCertificateSha256Hex = try decodeCanonicalVpnTrustHex(
            from: container,
            forKey: .relayCertificateSha256Hex,
            field: "vpn session relay_certificate_sha256_hex"
        )
        directorySnapshotDigestHex = try decodeCanonicalVpnTrustHex(
            from: container,
            forKey: .directorySnapshotDigestHex,
            field: "vpn session directory_snapshot_digest_hex"
        )
        routePushes = try container.decode([String].self, forKey: .routePushes)
        excludedRoutes = try container.decode([String].self, forKey: .excludedRoutes)
        dnsServers = try container.decode([String].self, forKey: .dnsServers)
        tunnelAddresses = try container.decode([String].self, forKey: .tunnelAddresses)
        mtuBytes = try decodeExactVpnUInt64(
            from: container,
            forKey: .mtuBytes,
            expected: 1_280,
            field: "vpn session mtu_bytes"
        )
        helperTicketHex = try decodeCanonicalVpnHex(
            from: container,
            forKey: .helperTicketHex,
            byteCount: 788,
            field: "vpn session helper_ticket_hex"
        )
        bytesIn = try decodeVpnUInt64(
            from: container,
            forKey: .bytesIn,
            field: "vpn session bytes_in"
        )
        bytesOut = try decodeVpnUInt64(
            from: container,
            forKey: .bytesOut,
            field: "vpn session bytes_out"
        )
        status = try decodeVpnEnum(
            from: container,
            forKey: .status,
            allowed: ["active"],
            field: "vpn session status"
        )
    }
}

public struct ToriiVpnReceiptSubmitRequest: Encodable, Sendable, Equatable {
    public var relayReceiptHex: String
    public var clientVoucherHex: String
    public var leaseIdHex: String?

    private enum CodingKeys: String, CodingKey {
        case relayReceiptHex = "relay_receipt_hex"
        case clientVoucherHex = "client_voucher_hex"
        case leaseIdHex = "lease_id_hex"
    }

    public init(relayReceiptHex: String,
                clientVoucherHex: String,
                leaseIdHex: String? = nil) {
        self.relayReceiptHex = relayReceiptHex
        self.clientVoucherHex = clientVoucherHex
        self.leaseIdHex = leaseIdHex
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(try ToriiRequestValidation.normalizedEvenLengthHex(relayReceiptHex,
                                                                                field: "relay_receipt_hex"),
                             forKey: .relayReceiptHex)
        try container.encode(try ToriiRequestValidation.normalizedEvenLengthHex(clientVoucherHex,
                                                                                field: "client_voucher_hex"),
                             forKey: .clientVoucherHex)
        try container.encodeIfPresent(try ToriiRequestValidation.normalizedOptional32ByteHex(leaseIdHex,
                                                                                            field: "lease_id_hex"),
                                      forKey: .leaseIdHex)
    }
}

public struct ToriiVpnReceipt: Decodable, Sendable, Equatable {
    public let sessionId: String
    public let accountId: String
    public let exitClass: String
    public let relayEndpoint: String
    public let meterFamily: String
    public let connectedAtMilliseconds: UInt64
    public let disconnectedAtMilliseconds: UInt64
    public let durationMilliseconds: UInt64
    public let bytesIn: UInt64
    public let bytesOut: UInt64
    public let status: String
    public let receiptSource: String
    public let quoteId: String
    public let paymentTransactionHash: String
    public let feeAssetId: String
    public let escrowAccountId: String
    public let operatorAccountId: String
    public let leaseFee: String
    public let earnedFee: String
    public let refundedFee: String
    public let leaseIdHex: String
    public let settleLeaseInstruction: ToriiVpnTxInstruction?

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case sessionId = "session_id"
        case accountId = "account_id"
        case exitClass = "exit_class"
        case relayEndpoint = "relay_endpoint"
        case meterFamily = "meter_family"
        case connectedAtMilliseconds = "connected_at_ms"
        case disconnectedAtMilliseconds = "disconnected_at_ms"
        case durationMilliseconds = "duration_ms"
        case bytesIn = "bytes_in"
        case bytesOut = "bytes_out"
        case status
        case receiptSource = "receipt_source"
        case quoteId = "quote_id"
        case paymentTransactionHash = "payment_tx_hash"
        case feeAssetId = "fee_asset_id"
        case escrowAccountId = "escrow_account_id"
        case operatorAccountId = "operator_account_id"
        case leaseFee = "lease_fee"
        case earnedFee = "earned_fee"
        case refundedFee = "refunded_fee"
        case leaseIdHex = "lease_id_hex"
        case settleLeaseInstruction = "settle_lease_instruction"
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownVpnFields(
            from: decoder,
            allowedKeys: CodingKeys.allCases,
            context: "vpn receipt response"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        sessionId = try decodeCanonicalVpnHex(
            from: container,
            forKey: .sessionId,
            byteCount: 16,
            field: "vpn receipt session_id"
        )
        accountId = try decodeNonEmptyVpnString(
            from: container,
            forKey: .accountId,
            field: "vpn receipt account_id"
        )
        exitClass = try decodeVpnEnum(
            from: container,
            forKey: .exitClass,
            allowed: toriiVpnExitClasses,
            field: "vpn receipt exit_class"
        )
        relayEndpoint = try decodeNonEmptyVpnString(
            from: container,
            forKey: .relayEndpoint,
            field: "vpn receipt relay_endpoint"
        )
        meterFamily = try decodeNonEmptyVpnString(
            from: container,
            forKey: .meterFamily,
            field: "vpn receipt meter_family"
        )
        connectedAtMilliseconds = try decodeVpnUInt64(
            from: container,
            forKey: .connectedAtMilliseconds,
            field: "vpn receipt connected_at_ms"
        )
        disconnectedAtMilliseconds = try decodeVpnUInt64(
            from: container,
            forKey: .disconnectedAtMilliseconds,
            field: "vpn receipt disconnected_at_ms"
        )
        durationMilliseconds = try decodeVpnUInt64(
            from: container,
            forKey: .durationMilliseconds,
            field: "vpn receipt duration_ms"
        )
        bytesIn = try decodeVpnUInt64(
            from: container,
            forKey: .bytesIn,
            field: "vpn receipt bytes_in"
        )
        bytesOut = try decodeVpnUInt64(
            from: container,
            forKey: .bytesOut,
            field: "vpn receipt bytes_out"
        )
        status = try decodeVpnEnum(
            from: container,
            forKey: .status,
            allowed: ["disconnected", "expired", "replaced", "settlement_pending", "settled"],
            field: "vpn receipt status"
        )
        receiptSource = try decodeVpnEnum(
            from: container,
            forKey: .receiptSource,
            allowed: ["torii", "relay", "wsv"],
            field: "vpn receipt receipt_source"
        )
        quoteId = try decodeCanonicalVpnHex(
            from: container,
            forKey: .quoteId,
            byteCount: 32,
            field: "vpn receipt quote_id"
        )
        paymentTransactionHash = try decodeCanonicalVpnHex(
            from: container,
            forKey: .paymentTransactionHash,
            byteCount: 32,
            field: "vpn receipt payment_tx_hash"
        )
        feeAssetId = try decodeNonEmptyVpnString(
            from: container,
            forKey: .feeAssetId,
            field: "vpn receipt fee_asset_id"
        )
        escrowAccountId = try decodeNonEmptyVpnString(
            from: container,
            forKey: .escrowAccountId,
            field: "vpn receipt escrow_account_id"
        )
        operatorAccountId = try decodeNonEmptyVpnString(
            from: container,
            forKey: .operatorAccountId,
            field: "vpn receipt operator_account_id"
        )
        leaseFee = try decodeCanonicalToriiQuantity(
            container.decode(String.self, forKey: .leaseFee),
            field: "vpn receipt lease_fee"
        )
        earnedFee = try decodeCanonicalToriiQuantity(
            container.decode(String.self, forKey: .earnedFee),
            field: "vpn receipt earned_fee"
        )
        refundedFee = try decodeCanonicalToriiQuantity(
            container.decode(String.self, forKey: .refundedFee),
            field: "vpn receipt refunded_fee"
        )
        leaseIdHex = try decodeCanonicalVpnHex(
            from: container,
            forKey: .leaseIdHex,
            byteCount: 32,
            field: "vpn receipt lease_id_hex"
        )
        settleLeaseInstruction = try container.decodeIfPresent(ToriiVpnTxInstruction.self,
                                                               forKey: .settleLeaseInstruction)
    }
}

public struct ToriiVpnReceiptListResponse: Decodable, Sendable, Equatable {
    public let items: [ToriiVpnReceipt]
    public let total: UInt64

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case items
        case total
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownVpnFields(
            from: decoder,
            allowedKeys: CodingKeys.allCases,
            context: "vpn receipt list response"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        items = try container.decode([ToriiVpnReceipt].self, forKey: .items)
        total = try decodeVpnUInt64(
            from: container,
            forKey: .total,
            field: "vpn receipt list total"
        )
        guard items.count <= 24, total <= 24 else {
            throw DecodingError.dataCorruptedError(
                forKey: .items,
                in: container,
                debugDescription: "vpn receipt list items and total must not exceed 24."
            )
        }
    }
}
