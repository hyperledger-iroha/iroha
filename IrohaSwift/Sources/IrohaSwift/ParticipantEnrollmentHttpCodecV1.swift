// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
import Foundation
import CryptoKit

/// Exact public enrollment message/header grammar. These types never establish a Native
/// owner, a certified current S/W cut, a FI customer, an issuer or an installation.
/// Dispatch still requires the finite Native owner before/after actual transport.
/// Authorization and returned header values are runtime originals: never log or persist them.
public enum ParticipantEnrollmentHttpCodecV1 {
    public enum Operation: UInt8 {
        case prepare = 1, rawAttestation = 2, certificate = 3
        public var pathSuffix: String {
            switch self {
            case .prepare: return "/v1/kagemusha/enrollment/ordinary/prepare"
            case .rawAttestation: return "/v1/kagemusha/enrollment/ordinary/raw-attestation"
            case .certificate: return "/v1/kagemusha/enrollment/ordinary/certificate"
            }
        }
    }
    public enum CodecError: Error { case invalidOriginal }

    /// Primitive independently selected context, not a verified or decoded authority owner.
    public final class Context {
        public let networkID: NetworkId
        public let authenticationNamespace: String
        public let actorID: String
        public let operation: Operation
        public let target: URL
        fileprivate let origin: String
        fileprivate let path: String
        fileprivate init(networkID: NetworkId, authenticationNamespace: String, actorID: String,
            operation: Operation, target: URL, origin: String, path: String) {
            self.networkID = networkID; self.authenticationNamespace = authenticationNamespace
            self.actorID = actorID; self.operation = operation; self.target = target
            self.origin = origin; self.path = path
        }
    }
    /// Exact unsigned data original; public controller checks are not current ledger admission.
    public final class Request {
        public let context: Context
        public let signatoryI105: String
        public let walletI105: String
        public let requestID: String
        public let idempotencyKey: String
        public let timestampMS: UInt64
        public let nonce: String
        fileprivate let signatory: AccountAddress
        fileprivate let wallet: AccountAddress
        private let body: Data
        private let session: Data
        fileprivate init(context: Context, signatoryI105: String, walletI105: String, requestID: String,
            idempotencyKey: String, timestampMS: UInt64, nonce: String, signatory: AccountAddress,
            wallet: AccountAddress, body: Data, session: Data) {
            self.context = context; self.signatoryI105 = signatoryI105; self.walletI105 = walletI105
            self.requestID = requestID; self.idempotencyKey = idempotencyKey
            self.timestampMS = timestampMS; self.nonce = nonce; self.signatory = signatory; self.wallet = wallet
            self.body = Data([UInt8](body)); self.session = Data([UInt8](session))
        }
        public func originalBody() -> Data { Data([UInt8](body)) }
        public func sessionSHA256() -> Data { Data([UInt8](session)) }
        public func signingMessage() throws -> Data { try ParticipantEnrollmentHttpCodecV1.message(self) }
    }
    /// One flattened actual HTTP value. An array retains duplicate values which a dictionary hides.
    public final class Header {
        public let name: String
        private let original: Data
        public init(name: String, value: Data) throws {
            // Unrelated values remain ignored by decode; bound known originals before any snapshot.
            guard !ParticipantEnrollmentHttpCodecV1.names.contains(name.lowercased()) || value.count <= 8192 else {
                throw CodecError.invalidOriginal
            }
            self.name = name; self.original = Data([UInt8](value))
        }
        public func originalValue() -> Data { Data([UInt8](original)) }
        fileprivate func enrollmentValue() throws -> Data {
            guard original.count <= 8192 else { throw CodecError.invalidOriginal }
            return Data([UInt8](original))
        }
    }
    /// Signature-checked data only; this decoder cannot construct any Native/FI verified request.
    public final class ReceivedOriginal {
        public let request: Request
        private let signature: Data
        fileprivate init(request: Request, signature: Data) { self.request = request; self.signature = Data([UInt8](signature)) }
        public func originalSignature() -> Data { Data([UInt8](signature)) }
    }
    private static let names = ["x-iroha-enrollment-contract", "x-iroha-enrollment-signatory",
        "x-iroha-enrollment-wallet", "x-iroha-enrollment-request-id", "idempotency-key",
        "x-iroha-enrollment-timestamp", "x-iroha-enrollment-nonce", "x-iroha-enrollment-signature",
        "authorization"]
    private static let outerDomain = Data("iroha.participant.ordinary-enrollment-request.v1\0".utf8)
    private static let networkDomain = Data("iroha.app.request.network.v1\0".utf8)
    public static func headerNames() -> [String] { names }

    /// Accept only the canonical serialized Rust HTTPS target from the actual selected mount.
    /// Refuse other spellings; never normalize or reconstruct a target from offered headers.
    public static func context(networkID: NetworkId, authenticationNamespace: String, actorID: String,
        operation: Operation, target: URL) throws -> Context {
        try visible(authenticationNamespace, maximum: 256); try visible(actorID, maximum: 256)
        let literal = target.absoluteString
        guard literal.utf8.allSatisfy({ (0x21...0x7e).contains($0) }),
            let components = URLComponents(url: target, resolvingAgainstBaseURL: false),
            components.scheme == "https", let host = components.host, !host.isEmpty,
            host == host.lowercased(), components.user == nil, components.password == nil,
            components.query == nil, components.fragment == nil else { throw invalid() }
        let port = components.port
        guard port != 443, port == nil || (0...65535).contains(port!) else { throw invalid() }
        let renderedHost = host.contains(":") && !host.hasPrefix("[") ? "[\(host)]" : host
        let authority = renderedHost + (port.map { ":\($0)" } ?? "")
        let path = components.percentEncodedPath
        guard path.hasPrefix("/"), path.utf8.count <= 64 * 1024, path.hasSuffix(operation.pathSuffix),
            literal == "https://\(authority)\(path)",
            !path.split(separator: "/", omittingEmptySubsequences: false).contains(where: {
                let segment = $0.lowercased().replacingOccurrences(of: "%2e", with: ".")
                return segment == "." || segment == ".."
            }) else { throw invalid() }
        return Context(networkID: networkID, authenticationNamespace: authenticationNamespace,
            actorID: actorID, operation: operation, target: target, origin: "https://\(authority)", path: path)
    }

    public static func originalRequest(context: Context, signatoryI105: String, walletI105: String,
        requestID: String, idempotencyKey: String, timestampMS: UInt64, nonce: String,
        originalBody: Data, sessionSHA256: Data) throws -> Request {
        guard (1...256 * 1024).contains(originalBody.count), sessionSHA256.count == 32 else { throw invalid() }
        let body = Data([UInt8](originalBody)), session = Data([UInt8](sessionSHA256))
        try visible(requestID, maximum: 128); try visible(idempotencyKey, maximum: 256); try canonicalNonce(nonce)
        guard timestampMS > 30_000, timestampMS <= UInt64.max - 30_000,
            (1...256 * 1024).contains(body.count), session.count == 32, session.contains(where: { $0 != 0 }) else { throw invalid() }
        let s = try account(signatoryI105), w = try account(walletI105)
        guard let key = s.singleControllerInfo(), key.algorithm == .ed25519, key.publicKey.count == 32,
            let policy = try w.multisigPolicyInfo(), policy.threshold == 1, policy.members.count == 1,
            policy.members[0].weight == 1, policy.members[0].algorithm == "ed25519",
            policy.members[0].publicKeyHex.lowercased() == "0x" + hex(key.publicKey),
            try s.canonicalBytes() != w.canonicalBytes() else { throw invalid() }
        return Request(context: context, signatoryI105: signatoryI105, walletI105: walletI105,
            requestID: requestID, idempotencyKey: idempotencyKey, timestampMS: timestampMS,
            nonce: nonce, signatory: s, wallet: w, body: body, session: session)
    }
    public static func authorizationDigest(_ authorization: Data) throws -> Data {
        guard (1...8192).contains(authorization.count) else { throw invalid() }
        let snapshot = Data([UInt8](authorization))
        guard (1...8192).contains(snapshot.count), snapshot.allSatisfy({ (0x20...0x7e).contains($0) }) else { throw invalid() }
        return Data(SHA256.hash(data: snapshot))
    }
    /// Check and retain the real existing raw Ed64; invoke no signer or Native custody constructor.
    public static func encodeHeaders(request: Request, originalSignature: Data, authorization: Data) throws -> [Header] {
        guard originalSignature.count == 64, (1...8192).contains(authorization.count) else { throw invalid() }
        let signature = Data([UInt8](originalSignature)), session = Data([UInt8](authorization))
        guard try authorizationDigest(session) == request.sessionSHA256() else { throw invalid() }
        try verify(request, signature: signature)
        let values = ["1", request.signatoryI105, request.walletI105, request.requestID,
            request.idempotencyKey, String(request.timestampMS), request.nonce, hex(signature)]
        let metadata = try values.enumerated().map { try Header(name: names[$0.offset], value: Data($0.element.utf8)) }
        return metadata + [try Header(name: "authorization", value: session)]
    }
    /// Pass every value before generic bearer `.get`. Actual FI session/current admission is separate.
    public static func decode(context: Context, actualMethod: String, actualRequestTarget: String,
        originalBody: Data, headers: [Header]) throws -> ReceivedOriginal {
        guard (1...256 * 1024).contains(originalBody.count) else { throw invalid() }
        let body = Data([UInt8](originalBody))
        guard actualMethod == "POST", actualRequestTarget == context.path else { throw invalid() }
        var values = [Data?](repeating: nil, count: 9)
        for header in headers {
            let name = header.name.lowercased()
            if let index = names.firstIndex(of: name) {
                let value = try header.enrollmentValue()
                guard values[index] == nil, value.count <= 8192 else { throw invalid() }
                values[index] = value
            } else if name.hasPrefix("x-iroha-enrollment-") { throw invalid() }
        }
        func value(_ index: Int) throws -> Data { guard let value = values[index] else { throw invalid() }; return value }
        func text(_ index: Int) throws -> String { guard let text = String(data: try value(index), encoding: .utf8) else { throw invalid() }; return text }
        guard try text(0) == "1" else { throw invalid() }
        let signature = try unhex(text(7), bytes: 64)
        let request = try originalRequest(context: context, signatoryI105: text(1), walletI105: text(2),
            requestID: text(3), idempotencyKey: text(4), timestampMS: decimal(text(5)), nonce: text(6),
            originalBody: body, sessionSHA256: authorizationDigest(value(8)))
        try verify(request, signature: signature)
        return ReceivedOriginal(request: request, signature: signature)
    }
    private static func message(_ request: Request) throws -> Data {
        let c = request.context
        var canonical = networkDomain; canonical.append(c.networkID.bytes)
        canonical.append(Data("POST\n\(c.path)\n\n\(hex(Data(SHA256.hash(data: request.originalBody()))))\n\(request.timestampMS)\n\(request.nonce)".utf8))
        var out = outerDomain; out.append(c.operation.rawValue)
        let fields = [Data(c.authenticationNamespace.utf8), Data(c.actorID.utf8),
            Data(try request.signatory.canonicalHex().utf8), Data(try request.wallet.canonicalHex().utf8),
            Data(c.origin.utf8), request.sessionSHA256(), Data(request.requestID.utf8),
            Data(request.idempotencyKey.utf8), canonical]
        for field in fields {
            guard let count = UInt32(exactly: field.count) else { throw invalid() }
            for shift in 0..<4 { out.append(UInt8(truncatingIfNeeded: count >> (8 * shift))) }
            out.append(field)
        }
        return out
    }
    private static func verify(_ request: Request, signature: Data) throws {
        guard Ed25519SignatureAdmission.isValidSignature(signature),
            let key = request.signatory.singleControllerInfo(), key.algorithm == .ed25519 else { throw invalid() }
        let verifier = try Curve25519.Signing.PublicKey(rawRepresentation: key.publicKey)
        // Rust Signature::try_new/verify are raw Ed25519 over the full subject, without prehash.
        guard verifier.isValidSignature(signature, for: try request.signingMessage()) else { throw invalid() }
    }
    private static func account(_ raw: String) throws -> AccountAddress {
        guard (1...4096).contains(raw.utf8.count) else { throw invalid() }
        let account = try AccountAddress.parseEncoded(raw, expectedPrefix: AccountId.defaultNetworkPrefix)
        guard try account.toI105(networkPrefix: AccountId.defaultNetworkPrefix) == raw else { throw invalid() }
        return account
    }
    private static func visible(_ raw: String, maximum: Int) throws {
        guard (1...maximum).contains(raw.utf8.count), raw.utf8.allSatisfy({ (0x21...0x7e).contains($0) }) else { throw invalid() }
    }
    private static func canonicalNonce(_ raw: String) throws {
        guard raw.utf8.count == 64, raw.utf8.allSatisfy({ (0x30...0x39).contains($0) || (0x61...0x66).contains($0) }) else { throw invalid() }
    }
    private static func decimal(_ raw: String) throws -> UInt64 {
        guard (1...20).contains(raw.utf8.count), raw.utf8.allSatisfy({ (0x30...0x39).contains($0) }),
            raw.count == 1 || !raw.hasPrefix("0"), let value = UInt64(raw), String(value) == raw else { throw invalid() }
        return value
    }
    private static func unhex(_ raw: String, bytes: Int) throws -> Data {
        guard raw.utf8.count == bytes * 2, raw.utf8.allSatisfy({ (0x30...0x39).contains($0) || (0x61...0x66).contains($0) }) else { throw invalid() }
        let chars = Array(raw.utf8); var out = Data(); out.reserveCapacity(bytes)
        for i in 0..<bytes {
            let pair = String(decoding: chars[(i * 2)..<(i * 2 + 2)], as: UTF8.self)
            guard let value = UInt8(pair, radix: 16) else { throw invalid() }; out.append(value)
        }
        return out
    }
    private static func hex(_ raw: Data) -> String { raw.map { String(format: "%02x", $0) }.joined() }
    private static func invalid() -> CodecError { .invalidOriginal }
}
