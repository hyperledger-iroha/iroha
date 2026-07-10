import CoreFoundation
import Foundation

/// Closed bridge payload kinds admitted in SCCP V1.
public enum SccpPayloadKindV1: String, CaseIterable, Sendable {
    case assetRegister = "asset_register"
    case routeActivate = "route_activate"
    case transfer
    case tokenAdd = "token_add"
    case tokenPause = "token_pause"
    case tokenResume = "token_resume"
}

/// Request payload for `POST /v1/bridge/proofs/submit`.
public struct ToriiBridgeProofSubmitRequest: Encodable, Equatable, Sendable {
    public let authority: String
    public let publicKeyHex: String?
    public let signatureB64: String?
    public let messageBundleB64: String
    public let networkIdHex: String?
    public let verifierAddressHex: String?
    public let bridgeAddressHex: String?
    public let verifierCodeHashHex: String?
    public let verifierKeyHashHex: String?
    public let tronVerifierAddress: String?
    public let proofBytesHex: String?
    public let creationTimeMs: UInt64?

    public init(
        authority: String,
        messageBundleB64: String,
        publicKeyHex: String? = nil,
        signatureB64: String? = nil,
        networkIdHex: String? = nil,
        verifierAddressHex: String? = nil,
        bridgeAddressHex: String? = nil,
        verifierCodeHashHex: String? = nil,
        verifierKeyHashHex: String? = nil,
        tronVerifierAddress: String? = nil,
        proofBytesHex: String? = nil,
        creationTimeMs: UInt64? = nil
    ) throws {
        self.authority = try SccpSubmitValidation.authority(authority)
        (self.publicKeyHex, self.signatureB64) = try SccpSubmitValidation.detachedSigner(
            publicKeyHex: publicKeyHex,
            signatureB64: signatureB64,
            authority: self.authority
        )
        _ = try SccpSubmitValidation.canonicalNoritoBase64(
            messageBundleB64,
            field: "message_bundle_b64"
        )
        self.messageBundleB64 = messageBundleB64
        self.networkIdHex = try SccpSubmitValidation.optionalHex(networkIdHex, bytes: 32, field: "network_id_hex")
        self.verifierAddressHex = try SccpSubmitValidation.optionalHex(verifierAddressHex, bytes: 20, field: "verifier_address_hex")
        self.bridgeAddressHex = try SccpSubmitValidation.optionalHex(bridgeAddressHex, bytes: 20, field: "bridge_address_hex")
        self.verifierCodeHashHex = try SccpSubmitValidation.optionalHex(verifierCodeHashHex, bytes: 32, field: "verifier_code_hash_hex")
        self.verifierKeyHashHex = try SccpSubmitValidation.optionalHex(verifierKeyHashHex, bytes: 32, field: "verifier_key_hash_hex")
        self.tronVerifierAddress = try SccpSubmitValidation.optionalCanonicalText(
            tronVerifierAddress,
            field: "tron_verifier_address",
            maximumUtf8Bytes: 128
        )
        self.proofBytesHex = try SccpSubmitValidation.optionalProofHex(proofBytesHex)
        if let creationTimeMs, creationTimeMs == 0 {
            throw SccpV1Error.invalid("creation_time_ms must be positive")
        }
        self.creationTimeMs = creationTimeMs

        let evm = self.verifierAddressHex != nil || self.bridgeAddressHex != nil
        let tron = self.tronVerifierAddress != nil
        let destination = self.networkIdHex != nil || evm || tron ||
            self.verifierCodeHashHex != nil || self.verifierKeyHashHex != nil
        guard (self.proofBytesHex != nil) == destination else {
            throw SccpV1Error.invalid("proof_bytes_hex and complete destination material must be supplied together")
        }
        if destination {
            guard evm != tron,
                  self.networkIdHex != nil,
                  self.verifierCodeHashHex != nil,
                  self.verifierKeyHashHex != nil
            else {
                throw SccpV1Error.invalid("destination material must select exactly one complete EVM or TRON family")
            }
            if evm, self.verifierAddressHex == nil || self.bridgeAddressHex == nil {
                throw SccpV1Error.invalid("complete EVM SCCP destination material is required")
            }
        }
    }

    private enum CodingKeys: String, CodingKey {
        case authority
        case publicKeyHex = "public_key_hex"
        case signatureB64 = "signature_b64"
        case messageBundleB64 = "message_bundle_b64"
        case networkIdHex = "network_id_hex"
        case verifierAddressHex = "verifier_address_hex"
        case bridgeAddressHex = "bridge_address_hex"
        case verifierCodeHashHex = "verifier_code_hash_hex"
        case verifierKeyHashHex = "verifier_key_hash_hex"
        case tronVerifierAddress = "tron_verifier_address"
        case proofBytesHex = "proof_bytes_hex"
        case creationTimeMs = "creation_time_ms"
    }
}

/// Native-proof-only request for `POST /v1/bridge/messages`.
public struct ToriiBridgeMessageSubmitRequest: Encodable, Equatable, Sendable {
    public let authority: String
    public let publicKeyHex: String?
    public let signatureB64: String?
    public let nativeProofB64: String
    public let creationTimeMs: UInt64?

    public init(
        authority: String,
        nativeProofB64: String,
        publicKeyHex: String? = nil,
        signatureB64: String? = nil,
        creationTimeMs: UInt64? = nil
    ) throws {
        self.authority = try SccpSubmitValidation.authority(authority)
        (self.publicKeyHex, self.signatureB64) = try SccpSubmitValidation.detachedSigner(
            publicKeyHex: publicKeyHex,
            signatureB64: signatureB64,
            authority: self.authority
        )
        _ = try SccpSubmitValidation.canonicalNoritoBase64(nativeProofB64, field: "native_proof_b64")
        self.nativeProofB64 = nativeProofB64
        if let creationTimeMs, creationTimeMs == 0 {
            throw SccpV1Error.invalid("creation_time_ms must be positive")
        }
        self.creationTimeMs = creationTimeMs
    }

    private enum CodingKeys: String, CodingKey {
        case authority
        case publicKeyHex = "public_key_hex"
        case signatureB64 = "signature_b64"
        case nativeProofB64 = "native_proof_b64"
        case creationTimeMs = "creation_time_ms"
    }
}

/// Request-bound fields that can be checked against a bridge submit response.
public struct SccpBridgeResponseExpectation: Equatable, Sendable {
    public let payloadKind: SccpPayloadKindV1?
    public let messageIdHex: String?
    public let counterpartyDomain: UInt32?
    public let counterpartyChain: SccpNetworkV1?
    public let creationTimeMs: UInt64?

    public init(
        payloadKind: SccpPayloadKindV1? = nil,
        messageIdHex: String? = nil,
        counterpartyDomain: UInt32? = nil,
        counterpartyChain: SccpNetworkV1? = nil,
        creationTimeMs: UInt64? = nil
    ) throws {
        self.payloadKind = payloadKind
        self.messageIdHex = try messageIdHex.map {
            try SccpSubmitValidation.responseHash($0, field: "expected message_id_hex")
        }
        if let counterpartyDomain, !(1...5).contains(counterpartyDomain) {
            throw SccpV1Error.invalid("expected counterparty_domain must be in 1...5")
        }
        if let counterpartyChain, !counterpartyChain.isExternal {
            throw SccpV1Error.invalid("expected counterparty_chain must be external")
        }
        if let counterpartyDomain, let counterpartyChain,
           counterpartyDomain != counterpartyChain.domainId
        {
            throw SccpV1Error.invalid("expected counterparty profile/domain mismatch")
        }
        if let creationTimeMs, creationTimeMs == 0 {
            throw SccpV1Error.invalid("expected creation_time_ms must be positive")
        }
        self.counterpartyDomain = counterpartyDomain
        self.counterpartyChain = counterpartyChain
        self.creationTimeMs = creationTimeMs
    }
}

/// Exact unified two-phase response returned by both SCCP submit endpoints.
public struct SccpBridgeSubmitResponse: Equatable, Sendable {
    public let submitted: Bool
    public let payloadKind: SccpPayloadKindV1
    public let messageIdHex: String
    public let backend: String
    public let counterpartyDomain: UInt32
    public let counterpartyChain: SccpNetworkV1
    public let manifestHashHex: String
    public let rangeStartHeight: UInt64
    public let rangeEndHeight: UInt64
    public let creationTimeMs: UInt64
    public let txHashHex: String?
    public let transactionPayloadB64: String?
    public let signingMessageB64: String?

    /// Strictly parse the exact response, rejecting duplicate, unknown, retired, or missing fields.
    public static func parse(
        _ data: Data,
        expectation: SccpBridgeResponseExpectation? = nil
    ) throws -> Self {
        let object = try SccpStrictJSON.object(data, label: "bridge submit response")
        let fields: Set<String> = [
            "submitted", "payload_kind", "message_id_hex", "backend",
            "counterparty_domain", "counterparty_chain", "manifest_hash_hex",
            "range_start_height", "range_end_height", "creation_time_ms", "tx_hash_hex",
            "transaction_payload_b64", "signing_message_b64",
        ]
        try SccpStrictJSON.exactFields(object, fields, label: "bridge submit response")
        let submitted = try SccpStrictJSON.boolean(object, "submitted")
        guard let payloadKind = SccpPayloadKindV1(rawValue: try SccpStrictJSON.text(object, "payload_kind")) else {
            throw SccpV1Error.invalid("payload_kind is unknown or retired")
        }
        let messageId = try SccpSubmitValidation.responseHash(
            SccpStrictJSON.text(object, "message_id_hex"),
            field: "message_id_hex"
        )
        let backend = try SccpStrictJSON.text(object, "backend")
        let backendPattern = try NSRegularExpression(pattern: #"^bridge/[a-z0-9/_-]+$"#)
        guard backend.utf8.count <= 128,
              backendPattern.firstMatch(in: backend, range: NSRange(backend.startIndex..., in: backend)) != nil
        else {
            throw SccpV1Error.invalid("backend must be a canonical bridge backend label")
        }
        let domain = try SccpStrictJSON.uint32(object, "counterparty_domain", minimum: 1, maximum: 5)
        let chainKey = try SccpStrictJSON.text(object, "counterparty_chain")
        guard let chain = SccpNetworkV1(rawValue: chainKey), chain.isExternal, chain.domainId == domain else {
            throw SccpV1Error.invalid("counterparty_chain and counterparty_domain must identify one exact external network")
        }
        let manifestHash = try SccpSubmitValidation.responseHash(
            SccpStrictJSON.text(object, "manifest_hash_hex"),
            field: "manifest_hash_hex"
        )
        let start = try SccpStrictJSON.uint64(object, "range_start_height", minimum: 1)
        let end = try SccpStrictJSON.uint64(object, "range_end_height", minimum: start)
        let creation = try SccpStrictJSON.uint64(object, "creation_time_ms", minimum: 1)
        let txHash = try SccpStrictJSON.optionalText(object, "tx_hash_hex").map {
            try SccpSubmitValidation.responseHash($0, field: "tx_hash_hex")
        }
        let payloadB64 = try SccpStrictJSON.optionalText(object, "transaction_payload_b64")
        let signingB64 = try SccpStrictJSON.optionalText(object, "signing_message_b64")
        if submitted {
            guard txHash != nil, payloadB64 == nil, signingB64 == nil else {
                throw SccpV1Error.invalid("submitted SCCP response must contain tx_hash_hex and no signing payload")
            }
        } else {
            guard txHash == nil, let payloadB64, let signingB64 else {
                throw SccpV1Error.invalid("prepared SCCP response requires transaction_payload_b64 and signing_message_b64")
            }
            let payload = try SccpSubmitValidation.canonicalBase64(
                payloadB64,
                field: "transaction_payload_b64",
                maximumBytes: 16 * 1024 * 1024
            )
            let signing = try SccpSubmitValidation.canonicalBase64(
                signingB64,
                field: "signing_message_b64",
                exactBytes: 32
            )
            var prehash = Blake2b.hash256(payload)
            prehash[prehash.count - 1] |= 1
            guard signing == prehash else {
                throw SccpV1Error.invalid("signing_message_b64 must be the exact transaction-payload prehash")
            }
        }
        let response = Self(
            submitted: submitted,
            payloadKind: payloadKind,
            messageIdHex: messageId,
            backend: backend,
            counterpartyDomain: domain,
            counterpartyChain: chain,
            manifestHashHex: manifestHash,
            rangeStartHeight: start,
            rangeEndHeight: end,
            creationTimeMs: creation,
            txHashHex: txHash,
            transactionPayloadB64: payloadB64,
            signingMessageB64: signingB64
        )
        try response.validate(expectation)
        return response
    }

    private func validate(_ expectation: SccpBridgeResponseExpectation?) throws {
        guard let expectation else { return }
        let checks: [(Bool, String)] = [
            (expectation.payloadKind == nil || expectation.payloadKind == payloadKind, "payload_kind"),
            (expectation.messageIdHex == nil || expectation.messageIdHex == messageIdHex, "message_id_hex"),
            (expectation.counterpartyDomain == nil || expectation.counterpartyDomain == counterpartyDomain, "counterparty_domain"),
            (expectation.counterpartyChain == nil || expectation.counterpartyChain == counterpartyChain, "counterparty_chain"),
            (expectation.creationTimeMs == nil || expectation.creationTimeMs == creationTimeMs, "creation_time_ms"),
        ]
        if let failed = checks.first(where: { !$0.0 }) {
            throw SccpV1Error.invalid("bridge submit response.\(failed.1) does not match the request")
        }
    }
}

enum SccpSubmitValidation {
    static let maximumArtifactBytes = 16 * 1024 * 1024

    static func authority(_ value: String) throws -> String {
        guard !value.isEmpty, value == value.trimmingCharacters(in: .whitespacesAndNewlines),
              let address = try? AccountAddress.parseEncoded(value),
              let canonical = try? address.toI105(networkPrefix: 0x02f1),
              canonical == value
        else {
            throw SccpV1Error.invalid("authority must be a canonical AccountId")
        }
        return value
    }

    static func detachedSigner(
        publicKeyHex: String?,
        signatureB64: String?,
        authority: String
    ) throws -> (String?, String?) {
        guard (publicKeyHex == nil) == (signatureB64 == nil) else {
            throw SccpV1Error.invalid("public_key_hex and signature_b64 must be supplied together")
        }
        guard let publicKeyHex, let signatureB64 else { return (nil, nil) }
        guard publicKeyHex.count == 64,
              publicKeyHex.utf8.allSatisfy({ (48...57).contains($0) || (97...102).contains($0) }),
              let publicKey = Data(hexString: publicKeyHex),
              Ed25519PublicKeyAdmission.isValidPublicKey(publicKey)
        else {
            throw SccpV1Error.invalid("public_key_hex must be one canonical Ed25519 public key")
        }
        guard let address = try? AccountAddress.parseEncoded(authority),
              let controller = address.singleControllerInfo(),
              controller.algorithm == .ed25519,
              controller.publicKey == publicKey
        else {
            throw SccpV1Error.invalid("public_key_hex does not match authority")
        }
        let signature = try canonicalBase64(signatureB64, field: "signature_b64", exactBytes: 64)
        guard Ed25519SignatureAdmission.isValidSignature(signature), canonicalEd25519Scalar(signature.suffix(32)) else {
            throw SccpV1Error.invalid("signature_b64 must contain one canonical Ed25519 signature")
        }
        return (publicKeyHex, signatureB64)
    }

    static func canonicalNoritoBase64(_ value: String, field: String) throws -> Data {
        let data = try canonicalBase64(value, field: field, maximumBytes: maximumArtifactBytes)
        guard let frame = noritoDecodeFrame(data),
              frame.header.compression == .none,
              frame.header.schema.contains(where: { $0 != 0 }),
              frame.paddingLength == 0 || frame.paddingLength == 8,
              data.prefix(NoritoHeader.encodedLength) == frame.header.encode()
        else {
            throw SccpV1Error.invalid("\(field) must contain one canonical uncompressed Norito envelope")
        }
        return data
    }

    static func canonicalBase64(
        _ value: String,
        field: String,
        exactBytes: Int? = nil,
        maximumBytes: Int = maximumArtifactBytes
    ) throws -> Data {
        guard !value.isEmpty, value == value.trimmingCharacters(in: .whitespacesAndNewlines),
              let decoded = Data(base64Encoded: value),
              !decoded.isEmpty,
              decoded.count <= maximumBytes,
              decoded.base64EncodedString() == value
        else {
            throw SccpV1Error.invalid("\(field) must be canonical nonempty padded base64")
        }
        if let exactBytes, decoded.count != exactBytes {
            throw SccpV1Error.invalid("\(field) must contain exactly \(exactBytes) bytes")
        }
        return decoded
    }

    static func optionalHex(_ value: String?, bytes: Int, field: String) throws -> String? {
        guard let value else { return nil }
        guard value.count == 2 + bytes * 2, value.hasPrefix("0x"),
              value.dropFirst(2).utf8.allSatisfy({ (48...57).contains($0) || (97...102).contains($0) }),
              value.dropFirst(2).contains(where: { $0 != "0" })
        else {
            throw SccpV1Error.invalid("\(field) must be canonical lowercase nonzero 0x-prefixed \(bytes)-byte hex")
        }
        return value
    }

    static func responseHash(_ value: String, field: String) throws -> String {
        guard value.count == 64,
              value.utf8.allSatisfy({ (48...57).contains($0) || (97...102).contains($0) }),
              value.contains(where: { $0 != "0" })
        else {
            throw SccpV1Error.invalid("\(field) must be canonical lowercase nonzero 32-byte hex")
        }
        return value
    }

    static func optionalCanonicalText(
        _ value: String?,
        field: String,
        maximumUtf8Bytes: Int
    ) throws -> String? {
        guard let value else { return nil }
        guard !value.isEmpty, value == value.trimmingCharacters(in: .whitespacesAndNewlines),
              value.utf8.count <= maximumUtf8Bytes
        else {
            throw SccpV1Error.invalid("\(field) must be canonical nonempty text")
        }
        return value
    }

    static func optionalProofHex(_ value: String?) throws -> String? {
        guard let value else { return nil }
        guard value.count == 768,
              value.utf8.allSatisfy({ (48...57).contains($0) || (97...102).contains($0) }),
              value.contains(where: { $0 != "0" }),
              let proof = Data(hexString: value)
        else {
            throw SccpV1Error.invalid("proof_bytes_hex must be canonical lowercase nonzero 384-byte hex")
        }
        let version = proof.prefix(32)
        guard version.dropLast().allSatisfy({ $0 == 0 }), version.last == 1,
              proof[32..<64].contains(where: { $0 != 0 }),
              proof[64..<96].allSatisfy({ $0 == 0 }),
              proof[96..<128].contains(where: { $0 != 0 })
        else {
            throw SccpV1Error.invalid("proof_bytes_hex has invalid SCCP public inputs")
        }
        return value
    }

    private static func canonicalEd25519Scalar(_ scalar: Data.SubSequence) -> Bool {
        let order: [UInt8] = [
            0xed, 0xd3, 0xf5, 0x5c, 0x1a, 0x63, 0x12, 0x58,
            0xd6, 0x9c, 0xf7, 0xa2, 0xde, 0xf9, 0xde, 0x14,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10,
        ]
        let bytes = Array(scalar)
        for index in stride(from: 31, through: 0, by: -1) {
            if bytes[index] < order[index] { return true }
            if bytes[index] > order[index] { return false }
        }
        return false
    }
}

enum SccpStrictJSON {
    static func object(_ data: Data, label: String) throws -> [String: Any] {
        try rejectDuplicateKeys(data)
        guard let value = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            throw SccpV1Error.invalid("\(label) must be a JSON object")
        }
        return value
    }

    static func exactFields(_ value: [String: Any], _ fields: Set<String>, label: String) throws {
        if let unknown = value.keys.first(where: { !fields.contains($0) }) {
            throw SccpV1Error.invalid("\(label) contains unknown or retired field `\(unknown)`")
        }
        if let missing = fields.first(where: { value[$0] == nil }) {
            throw SccpV1Error.invalid("\(label) is missing required field `\(missing)`")
        }
    }

    static func text(_ value: [String: Any], _ field: String) throws -> String {
        guard let result = value[field] as? String, !result.isEmpty,
              result == result.trimmingCharacters(in: .whitespacesAndNewlines)
        else {
            throw SccpV1Error.invalid("\(field) must be canonical nonempty text")
        }
        return result
    }

    static func optionalText(_ value: [String: Any], _ field: String) throws -> String? {
        guard let raw = value[field], !(raw is NSNull) else { return nil }
        return try text(value, field)
    }

    static func boolean(_ value: [String: Any], _ field: String) throws -> Bool {
        guard let result = value[field] as? Bool else {
            throw SccpV1Error.invalid("\(field) must be boolean")
        }
        return result
    }

    static func uint32(
        _ value: [String: Any],
        _ field: String,
        minimum: UInt32,
        maximum: UInt32
    ) throws -> UInt32 {
        let number = try uint64(value, field, minimum: UInt64(minimum))
        guard number <= UInt64(maximum) else {
            throw SccpV1Error.invalid("\(field) is out of range")
        }
        return UInt32(number)
    }

    static func uint64(_ value: [String: Any], _ field: String, minimum: UInt64) throws -> UInt64 {
        guard let number = value[field] as? NSNumber,
              CFGetTypeID(number) != CFBooleanGetTypeID(),
              number.doubleValue.isFinite,
              number.doubleValue.rounded(.towardZero) == number.doubleValue,
              number.doubleValue >= 0,
              number.doubleValue <= Double(UInt64.max)
        else {
            throw SccpV1Error.invalid("\(field) must be an unsigned integer")
        }
        let result = number.uint64Value
        guard result >= minimum, number.doubleValue == Double(result) else {
            throw SccpV1Error.invalid("\(field) is out of range")
        }
        return result
    }

    private static func rejectDuplicateKeys(_ data: Data) throws {
        guard let text = String(data: data, encoding: .utf8), Data(text.utf8) == data else {
            throw SccpV1Error.invalid("JSON must be valid UTF-8")
        }
        var parser = DuplicateKeyParser(text)
        try parser.parse()
    }

    private struct DuplicateKeyParser {
        let text: String
        var index: String.Index

        init(_ text: String) {
            self.text = text
            index = text.startIndex
        }

        mutating func parse() throws {
            try value()
            whitespace()
            guard index == text.endIndex else { throw SccpV1Error.invalid("invalid JSON") }
        }

        mutating func value() throws {
            whitespace()
            guard let char = peek() else { throw SccpV1Error.invalid("invalid JSON") }
            switch char {
            case "{": try object()
            case "[": try array()
            case "\"": _ = try string()
            case "-", "0"..."9": try number()
            case "t": try consume("true")
            case "f": try consume("false")
            case "n": try consume("null")
            default: throw SccpV1Error.invalid("invalid JSON")
            }
        }

        mutating func object() throws {
            try consume("{")
            whitespace()
            var keys = Set<String>()
            if consumeIf("}") { return }
            while true {
                whitespace()
                let key = try string()
                guard keys.insert(key).inserted else {
                    throw SccpV1Error.invalid("JSON contains duplicate object key `\(key)`")
                }
                whitespace()
                try consume(":")
                try value()
                whitespace()
                if consumeIf("}") { return }
                try consume(",")
            }
        }

        mutating func array() throws {
            try consume("[")
            whitespace()
            if consumeIf("]") { return }
            while true {
                try value()
                whitespace()
                if consumeIf("]") { return }
                try consume(",")
            }
        }

        mutating func string() throws -> String {
            try consume("\"")
            var scalars = String.UnicodeScalarView()
            while let char = peek() {
                if char == "\"" {
                    advance()
                    return String(scalars)
                }
                if char == "\\" {
                    advance()
                    guard let escaped = peek() else { throw SccpV1Error.invalid("invalid JSON string") }
                    advance()
                    switch escaped {
                    case "\"", "\\", "/": scalars.append(escaped.unicodeScalars.first!)
                    case "b": scalars.append(UnicodeScalar(0x08)!)
                    case "f": scalars.append(UnicodeScalar(0x0c)!)
                    case "n": scalars.append(UnicodeScalar(0x0a)!)
                    case "r": scalars.append(UnicodeScalar(0x0d)!)
                    case "t": scalars.append(UnicodeScalar(0x09)!)
                    case "u": scalars.append(try unicodeEscape())
                    default: throw SccpV1Error.invalid("invalid JSON escape")
                    }
                } else {
                    guard char.unicodeScalars.allSatisfy({ $0.value >= 0x20 }) else {
                        throw SccpV1Error.invalid("invalid JSON string")
                    }
                    char.unicodeScalars.forEach { scalars.append($0) }
                    advance()
                }
            }
            throw SccpV1Error.invalid("unterminated JSON string")
        }

        mutating func unicodeEscape() throws -> UnicodeScalar {
            let high = try hexQuad()
            if (0xd800...0xdbff).contains(high) {
                guard consumeIf("\\"), consumeIf("u") else {
                    throw SccpV1Error.invalid("invalid JSON surrogate pair")
                }
                let low = try hexQuad()
                guard (0xdc00...0xdfff).contains(low),
                      let scalar = UnicodeScalar(0x10000 + ((high - 0xd800) << 10) + low - 0xdc00)
                else { throw SccpV1Error.invalid("invalid JSON surrogate pair") }
                return scalar
            }
            guard !(0xdc00...0xdfff).contains(high), let scalar = UnicodeScalar(high) else {
                throw SccpV1Error.invalid("invalid JSON Unicode escape")
            }
            return scalar
        }

        mutating func hexQuad() throws -> UInt32 {
            var result: UInt32 = 0
            for _ in 0..<4 {
                guard let char = peek(), let digit = char.hexDigitValue else {
                    throw SccpV1Error.invalid("invalid JSON Unicode escape")
                }
                result = result * 16 + UInt32(digit)
                advance()
            }
            return result
        }

        mutating func number() throws {
            _ = consumeIf("-")
            guard let first = peek(), first.isNumber else { throw SccpV1Error.invalid("invalid JSON number") }
            if first == "0" {
                advance()
                if let next = peek(), next.isNumber { throw SccpV1Error.invalid("invalid JSON number") }
            } else {
                while let char = peek(), char.isNumber { advance() }
            }
            if consumeIf(".") {
                guard let first = peek(), first.isNumber else { throw SccpV1Error.invalid("invalid JSON number") }
                while let char = peek(), char.isNumber { advance() }
            }
            if let char = peek(), char == "e" || char == "E" {
                advance()
                _ = consumeIf("+") || consumeIf("-")
                guard let first = peek(), first.isNumber else { throw SccpV1Error.invalid("invalid JSON number") }
                while let digit = peek(), digit.isNumber { advance() }
            }
        }

        mutating func whitespace() {
            while let char = peek(), " \n\r\t".contains(char) { advance() }
        }

        mutating func consume(_ literal: String) throws {
            guard text[index...].hasPrefix(literal) else { throw SccpV1Error.invalid("invalid JSON") }
            index = text.index(index, offsetBy: literal.count)
        }

        mutating func consumeIf(_ literal: String) -> Bool {
            guard text[index...].hasPrefix(literal) else { return false }
            index = text.index(index, offsetBy: literal.count)
            return true
        }

        func peek() -> Character? { index == text.endIndex ? nil : text[index] }
        mutating func advance() { index = text.index(after: index) }
    }
}
