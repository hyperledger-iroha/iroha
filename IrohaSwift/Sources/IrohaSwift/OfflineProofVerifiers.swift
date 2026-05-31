import CryptoKit
import Foundation
import Security

private enum OfflineProofCBORError: LocalizedError {
    case invalidFormat

    var errorDescription: String? {
        "Offline proof CBOR payload is invalid."
    }
}

public protocol CounterpartyOfflineProofVerifying {
    func verifyDeviceBinding(
        accountId: String,
        binding: ToriiOfflineDeviceBinding,
        expectedChallengeHashHex: String?
    ) throws

    func verifyDeviceProof(
        binding: ToriiOfflineDeviceBinding,
        proof: ToriiOfflineDeviceProof
    ) throws
}

public struct CounterpartyOfflineProofVerifier: CounterpartyOfflineProofVerifying {
    private let iosVerifier: IosOfflineProofVerifier
    private let androidVerifier: AndroidOfflineProofVerifier

    public init(
        iosVerifier: IosOfflineProofVerifier = IosOfflineProofVerifier(),
        androidVerifier: AndroidOfflineProofVerifier = AndroidOfflineProofVerifier()
    ) {
        self.iosVerifier = iosVerifier
        self.androidVerifier = androidVerifier
    }

    public func verifyDeviceBinding(
        accountId: String,
        binding: ToriiOfflineDeviceBinding,
        expectedChallengeHashHex: String?
    ) throws {
        switch binding.platform.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() {
        case "ios":
            guard let expectedChallengeHashHex else {
                throw OfflineProofVerifierError.invalidBinding("Missing offline device binding challenge hash.")
            }
            try iosVerifier.verifyDeviceBinding(
                accountId: accountId,
                binding: binding,
                expectedChallengeHashHex: expectedChallengeHashHex
            )
        case "android":
            try androidVerifier.verifyDeviceBinding(binding)
        default:
            throw OfflineProofVerifierError.invalidBinding("Unsupported offline device binding platform.")
        }
    }

    public func verifyDeviceProof(
        binding: ToriiOfflineDeviceBinding,
        proof: ToriiOfflineDeviceProof
    ) throws {
        switch binding.platform.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() {
        case "ios":
            try iosVerifier.verifyDeviceProof(binding: binding, proof: proof)
        case "android":
            try androidVerifier.verifyDeviceProof(binding: binding, proof: proof)
        default:
            throw OfflineProofVerifierError.invalidProof("Unsupported offline device proof platform.")
        }
    }
}

public enum OfflineProofVerifierError: LocalizedError {
    case invalidBinding(String)
    case invalidProof(String)

    public var errorDescription: String? {
        switch self {
        case .invalidBinding(let message), .invalidProof(let message):
            return message
        }
    }
}

public struct IosOfflineProofVerifier {
    private let trustedRoots: [SecCertificate]

    public init() {
        self.init(trustedRoots: Self.defaultTrustedRoots)
    }

    public init(trustedRoots: [SecCertificate]) {
        self.trustedRoots = trustedRoots
    }

    public func verifyDeviceBinding(
        accountId: String,
        binding: ToriiOfflineDeviceBinding,
        expectedChallengeHashHex: String
    ) throws {
        #if targetEnvironment(simulator)
        if binding.attestationReportBase64.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            return
        }
        #endif

        guard binding.platform.caseInsensitiveCompare(Self.platform) == .orderedSame else {
            throw OfflineProofVerifierError.invalidBinding("Unsupported offline device binding platform.")
        }
        guard !accountId.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw OfflineProofVerifierError.invalidBinding("Offline device binding account id is invalid.")
        }
        let metadata = try requireMetadata(binding)
        let attestationKeyIdBytes = try OfflineProofVerifierSupport.decodeCanonicalBase64(
            binding.attestationKeyId,
            error: "Offline device binding attestation key id is invalid."
        )
        let challengeHashBytes = try OfflineProofVerifierSupport.decodeHexDigest(
            expectedChallengeHashHex,
            error: "Offline device binding challenge hash is invalid."
        )
        let attestation = try decodeAttestationObject(binding.attestationReportBase64)
        let authData = try parseAttestationAuthData(attestation.authData)
        guard authData.signCount == 0 else {
            throw OfflineProofVerifierError.invalidBinding(
                "Offline device binding attestation counter must start at zero."
            )
        }
        guard authData.aaguid == expectedAaguid(environment: metadata.environment) else {
            throw OfflineProofVerifierError.invalidBinding(
                "Offline device binding App Attest environment is invalid."
            )
        }
        guard authData.credentialId == attestationKeyIdBytes else {
            throw OfflineProofVerifierError.invalidBinding(
                "Offline device binding attestation key id is invalid."
            )
        }
        guard authData.rpIdHash == expectedRpIdHash(metadata) else {
            throw OfflineProofVerifierError.invalidBinding(
                "Offline device binding app identity is invalid."
            )
        }
        let leaf = try OfflineProofVerifierSupport.verifyCertificateChain(
            certificates: attestation.certificates,
            trustedRoots: trustedRoots,
            untrustedChainMessage: "Offline device binding attestation report must include a certificate chain.",
            rootTrustMessage: "Offline device binding root certificate is not trusted."
        )[0]
        try verifyCredentialKeyIdentifier(
            certificate: leaf.certificate,
            expectedKeyId: attestationKeyIdBytes
        )
        try verifyNonce(
            certificateDER: leaf.der,
            authData: attestation.authData,
            clientDataHash: challengeHashBytes
        )
    }

    public func verifyDeviceProof(
        binding: ToriiOfflineDeviceBinding,
        proof: ToriiOfflineDeviceProof
    ) throws {
        #if targetEnvironment(simulator)
        if binding.attestationReportBase64.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            return
        }
        #endif

        guard proof.platform.caseInsensitiveCompare(Self.platform) == .orderedSame else {
            throw OfflineProofVerifierError.invalidProof("Unsupported offline device proof platform.")
        }
        let metadata = try requireMetadata(binding)
        let bindingKeyId = try OfflineProofVerifierSupport.decodeCanonicalBase64(
            binding.attestationKeyId,
            error: "Offline device binding attestation key id is invalid."
        )
        let proofKeyId = try OfflineProofVerifierSupport.decodeCanonicalBase64(
            proof.attestationKeyId,
            error: "Offline device proof does not match the device binding."
        )
        guard bindingKeyId == proofKeyId else {
            throw OfflineProofVerifierError.invalidProof(
                "Offline device proof does not match the device binding."
            )
        }
        guard let counter = proof.counter else {
            throw OfflineProofVerifierError.invalidProof("iOS offline proofs must include a counter.")
        }
        let assertion = try decodeAssertion(proof.assertionBase64)
        let challengeHashBytes = try OfflineProofVerifierSupport.decodeHexDigest(
            proof.challengeHashHex,
            error: "Offline device proof challenge hash is invalid."
        )
        guard assertion.rpIdHash == expectedRpIdHash(metadata) else {
            throw OfflineProofVerifierError.invalidProof("Offline device proof app identity is invalid.")
        }
        guard assertion.signCount == counter else {
            throw OfflineProofVerifierError.invalidProof("Offline device proof counter is invalid.")
        }
        let attestation = try decodeAttestationObject(binding.attestationReportBase64)
        let chain = try OfflineProofVerifierSupport.verifyCertificateChain(
            certificates: attestation.certificates,
            trustedRoots: trustedRoots,
            untrustedChainMessage: "Offline device binding attestation report must include a certificate chain.",
            rootTrustMessage: "Offline device binding root certificate is not trusted."
        )
        let attestationAuthData = try parseAttestationAuthData(attestation.authData)
        let cosePublicKey = try credentialPublicKey(from: attestationAuthData.credentialPublicKey)
        var publicKeys: [SecKey] = []
        appendUniquePublicKey(
            cosePublicKey,
            to: &publicKeys
        )
        appendUniquePublicKey(
            try certificatePublicKey(from: chain[0].certificate).key,
            to: &publicKeys
        )
        let valid = publicKeys.contains { publicKey in
            verifyAssertionSignature(
                publicKey: publicKey,
                authenticatorData: assertion.authenticatorData,
                challengeHashBytes: challengeHashBytes,
                signature: assertion.signature
            )
        }
        guard valid else {
            throw OfflineProofVerifierError.invalidProof("Offline device proof assertion is invalid.")
        }
    }

    private func requireMetadata(_ binding: ToriiOfflineDeviceBinding) throws -> IosMetadata {
        let teamId = binding.iosTeamId?.trimmingCharacters(in: .whitespacesAndNewlines).uppercased() ?? ""
        let bundleId = binding.iosBundleId?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        let environment = binding.iosEnvironment?.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() ?? ""
        guard !teamId.isEmpty, !bundleId.isEmpty else {
            throw OfflineProofVerifierError.invalidBinding(
                "Offline device binding iOS metadata is incomplete."
            )
        }
        let normalizedEnvironment: String
        switch environment {
        case Self.environmentDevelopment, Self.environmentProduction:
            normalizedEnvironment = environment
        default:
            throw OfflineProofVerifierError.invalidBinding(
                "Offline device binding iOS environment is invalid."
            )
        }
        return IosMetadata(teamId: teamId, bundleId: bundleId, environment: normalizedEnvironment)
    }

    private func decodeAttestationObject(_ value: String) throws -> AttestationObject {
        let bytes = try OfflineProofVerifierSupport.decodeBase64(
            value,
            error: "Offline device binding attestation report is invalid."
        )
        var reader = OfflineProofCBORReader(data: bytes)
        guard let map = try reader.readValue().mapValue, reader.isAtEnd else {
            throw OfflineProofVerifierError.invalidBinding(
                "Offline device binding attestation report is invalid."
            )
        }
        guard map[Self.keyFormat]?.textValue == Self.formatAppleAppAttest else {
            throw OfflineProofVerifierError.invalidBinding(
                "Offline device binding attestation report format is invalid."
            )
        }
        guard let authData = map[Self.keyAuthData]?.byteStringValue,
              let attStmt = map[Self.keyAttStmt]?.mapValue,
              let certificates = attStmt[Self.keyX5c]?.arrayValue?.map(\.byteStringValue).optionalUnwrap()
        else {
            throw OfflineProofVerifierError.invalidBinding(
                "Offline device binding attestation report is invalid."
            )
        }
        guard certificates.count >= 2 else {
            throw OfflineProofVerifierError.invalidBinding(
                "Offline device binding attestation report must include a certificate chain."
            )
        }
        return AttestationObject(authData: authData, certificates: certificates)
    }

    private func parseAttestationAuthData(_ authData: Data) throws -> AttestationAuthData {
        guard authData.count >= Self.attestationAuthDataPrefixSize else {
            throw OfflineProofVerifierError.invalidBinding(
                "Offline device binding attestation authData is too short."
            )
        }
        let flags = Int(authData[Self.flagsOffset])
        guard (flags & Self.flagAttestedCredentialData) != 0 else {
            throw OfflineProofVerifierError.invalidBinding(
                "Offline device binding attested credential data is missing."
            )
        }
        let signCount = try OfflineProofVerifierSupport.readUInt32(authData, offset: Self.signCountOffset)
        var offset = Self.attestationAuthDataFixedSize
        let aaguidEnd = offset + Self.aaguidSize
        guard aaguidEnd <= authData.count else {
            throw OfflineProofVerifierError.invalidBinding(
                "Offline device binding attestation authData is too short."
            )
        }
        let aaguid = authData.subdata(in: offset ..< aaguidEnd)
        offset = aaguidEnd
        let credentialIdLength = try OfflineProofVerifierSupport.readUInt16(authData, offset: offset)
        offset += 2
        let credentialIdEnd = offset + credentialIdLength
        guard credentialIdEnd <= authData.count else {
            throw OfflineProofVerifierError.invalidBinding(
                "Offline device binding credential id exceeds authData bounds."
            )
        }
        return AttestationAuthData(
            rpIdHash: authData.subdata(in: 0 ..< Self.rpIdHashSize),
            signCount: signCount,
            aaguid: aaguid,
            credentialId: authData.subdata(in: offset ..< credentialIdEnd),
            credentialPublicKey: authData.subdata(in: credentialIdEnd ..< authData.endIndex)
        )
    }

    private func credentialPublicKey(from coseKeyData: Data) throws -> SecKey {
        var reader = OfflineProofCBORReader(data: coseKeyData)
        guard let coseKey = try reader.readValue().intMapValue,
              coseKey[1]?.intValue == 2,
              coseKey[-1]?.intValue == 1,
              let x = coseKey[-2]?.byteStringValue,
              let y = coseKey[-3]?.byteStringValue,
              x.count == 32,
              y.count == 32
        else {
            throw OfflineProofVerifierError.invalidProof("Offline device proof assertion is invalid.")
        }
        if let algorithm = coseKey[3]?.intValue, algorithm != -7 {
            throw OfflineProofVerifierError.invalidProof("Offline device proof assertion is invalid.")
        }
        let keyData = OfflineProofVerifierSupport.concatenate(
            OfflineProofVerifierSupport.concatenate(Data([0x04]), x),
            y
        )
        let attributes: [CFString: Any] = [
            kSecAttrKeyType: kSecAttrKeyTypeECSECPrimeRandom,
            kSecAttrKeyClass: kSecAttrKeyClassPublic,
            kSecAttrKeySizeInBits: 256,
        ]
        var error: Unmanaged<CFError>?
        guard let key = SecKeyCreateWithData(keyData as CFData, attributes as CFDictionary, &error) else {
            throw OfflineProofVerifierError.invalidProof("Offline device proof assertion is invalid.")
        }
        return key
    }

    private func certificatePublicKey(from certificate: SecCertificate) throws -> (key: SecKey, x963: Data) {
        guard let key = SecCertificateCopyKey(certificate) else {
            throw OfflineProofVerifierError.invalidBinding("Offline device binding attestation key id is invalid.")
        }
        var error: Unmanaged<CFError>?
        guard let representation = SecKeyCopyExternalRepresentation(key, &error) as Data? else {
            throw OfflineProofVerifierError.invalidBinding("Offline device binding attestation key id is invalid.")
        }
        return (key, representation)
    }

    private func appendUniquePublicKey(_ key: SecKey, to keys: inout [SecKey]) {
        guard let candidate = publicKeyRepresentation(key) else {
            keys.append(key)
            return
        }
        let candidateHash = Data(SHA256.hash(data: candidate))
        let alreadyPresent = keys.contains { existing in
            guard let existing = publicKeyRepresentation(existing) else {
                return false
            }
            return Data(SHA256.hash(data: existing)) == candidateHash
        }
        if !alreadyPresent {
            keys.append(key)
        }
    }

    private func publicKeyRepresentation(_ key: SecKey) -> Data? {
        var error: Unmanaged<CFError>?
        return SecKeyCopyExternalRepresentation(key, &error) as Data?
    }

    private func verifyCredentialKeyIdentifier(certificate: SecCertificate, expectedKeyId: Data) throws {
        let publicKey = try certificatePublicKey(from: certificate)
        let actualKeyId = Data(SHA256.hash(data: publicKey.x963))
        guard actualKeyId == expectedKeyId else {
            throw OfflineProofVerifierError.invalidBinding("Offline device binding attestation key id is invalid.")
        }
    }

    private func verifyAssertionSignature(
        publicKey: SecKey,
        authenticatorData: Data,
        challengeHashBytes: Data,
        signature: Data
    ) -> Bool {
        let nonce = Data(
            SHA256.hash(
                data: OfflineProofVerifierSupport.concatenate(
                    authenticatorData,
                    challengeHashBytes
                )
            )
        )
        return verifyECDSASignature(publicKey: publicKey, message: nonce, signature: signature)
    }

    private func verifyECDSASignature(publicKey: SecKey, message: Data, signature: Data) -> Bool {
        var error: Unmanaged<CFError>?
        if SecKeyVerifySignature(
            publicKey,
            .ecdsaSignatureMessageX962SHA256,
            message as CFData,
            signature as CFData,
            &error
        ) {
            return true
        }

        let digest = Data(SHA256.hash(data: message))
        error = nil
        if SecKeyVerifySignature(
            publicKey,
            .ecdsaSignatureDigestX962SHA256,
            digest as CFData,
            signature as CFData,
            &error
        ) {
            return true
        }

        return verifyP256Signature(publicKey: publicKey, message: message, signature: signature)
    }

    private func verifyP256Signature(publicKey: SecKey, message: Data, signature: Data) -> Bool {
        guard let representation = publicKeyRepresentation(publicKey) else {
            return false
        }
        guard let p256PublicKey = try? P256.Signing.PublicKey(x963Representation: representation) else {
            return false
        }
        let signatures = [
            try? P256.Signing.ECDSASignature(derRepresentation: signature),
            try? P256.Signing.ECDSASignature(rawRepresentation: signature),
        ].compactMap { $0 }
        guard !signatures.isEmpty else {
            return false
        }
        let digest = SHA256.hash(data: message)
        return signatures.contains { candidate in
            p256PublicKey.isValidSignature(candidate, for: message) ||
                p256PublicKey.isValidSignature(candidate, for: digest)
        }
    }

    private func decodeAssertion(_ value: String) throws -> AssertionObject {
        let bytes = try OfflineProofVerifierSupport.decodeBase64(
            value,
            error: "Offline device proof assertion is invalid."
        )
        var reader = OfflineProofCBORReader(data: bytes)
        guard let map = try reader.readValue().mapValue, reader.isAtEnd else {
            throw OfflineProofVerifierError.invalidProof("Offline device proof assertion is invalid.")
        }
        guard let authenticatorData = map[Self.keyAuthenticatorData]?.byteStringValue,
              authenticatorData.count >= Self.assertionAuthDataSize,
              let signature = map[Self.keySignature]?.byteStringValue
        else {
            throw OfflineProofVerifierError.invalidProof("Offline device proof assertion is invalid.")
        }
        return AssertionObject(
            authenticatorData: authenticatorData,
            rpIdHash: authenticatorData.subdata(in: 0 ..< Self.rpIdHashSize),
            signCount: try OfflineProofVerifierSupport.readUInt32(
                authenticatorData,
                offset: Self.signCountOffset
            ),
            signature: signature
        )
    }

    private func verifyNonce(
        certificateDER: Data,
        authData: Data,
        clientDataHash: Data
    ) throws {
        let expectedNonce = Data(
            SHA256.hash(
                data: OfflineProofVerifierSupport.concatenate(authData, clientDataHash)
            )
        )
        let extensionValue = try OfflineProofVerifierSupport.extensionValue(
            oid: Self.appleNonceOID,
            certificateDER: certificateDER,
            missingMessage: "Offline device binding nonce extension is missing.",
            invalidMessage: "Offline device binding nonce extension is invalid."
        )
        let nonceNode = try OfflineProofVerifierSupport.parseSingleASN1Node(
            extensionValue,
            invalidMessage: "Offline device binding nonce extension is invalid."
        )
        guard nonceValue(from: nonceNode) == expectedNonce else {
            throw OfflineProofVerifierError.invalidBinding("Offline device binding nonce is invalid.")
        }
    }

    private func nonceValue(from node: OfflineProofASN1Node) -> Data? {
        if node.tagClass == .universal, node.tagNumber == 4 {
            return node.primitiveValue
        }
        if node.tagClass == .universal,
           node.tagNumber == 16,
           node.children.count == 1 {
            return nonceValue(from: node.children[0])
        }
        if node.tagClass == .contextSpecific,
           node.tagNumber == 1,
           node.children.count == 1 {
            return nonceValue(from: node.children[0])
        }
        return nil
    }

    private func expectedRpIdHash(_ metadata: IosMetadata) -> Data {
        let rpId = "\(metadata.teamId).\(metadata.bundleId)"
        return Data(SHA256.hash(data: Data(rpId.utf8)))
    }

    private func expectedAaguid(environment: String) -> Data {
        switch environment {
        case Self.environmentDevelopment:
            return Self.appleDevelopmentAaguid
        default:
            return Self.appleProductionAaguid
        }
    }

    private struct IosMetadata {
        let teamId: String
        let bundleId: String
        let environment: String
    }

    private struct AttestationObject {
        let authData: Data
        let certificates: [Data]
    }

    private struct AttestationAuthData {
        let rpIdHash: Data
        let signCount: UInt64
        let aaguid: Data
        let credentialId: Data
        let credentialPublicKey: Data
    }

    private struct AssertionObject {
        let authenticatorData: Data
        let rpIdHash: Data
        let signCount: UInt64
        let signature: Data
    }

    private static let defaultTrustedRoots: [SecCertificate] = {
        [OfflineProofVerifierSupport.pemCertificate(appleAppAttestationRootCAPEM)]
            .compactMap { $0 }
    }()

    public static var defaultTrustedRootCertificateBase64: String {
        guard let root = defaultTrustedRoots.first else {
            return ""
        }
        return (SecCertificateCopyData(root) as Data).base64EncodedString()
    }

    private static let appleProductionAaguid = Data("appattest\0\0\0\0\0\0\0".utf8)
    private static let appleDevelopmentAaguid = Data("appattestdevelop".utf8)
    private static let appleAppAttestationRootCAPEM = """
    -----BEGIN CERTIFICATE-----
    MIICITCCAaegAwIBAgIQC/O+DvHN0uD7jG5yH2IXmDAKBggqhkjOPQQDAzBSMSYw
    JAYDVQQDDB1BcHBsZSBBcHAgQXR0ZXN0YXRpb24gUm9vdCBDQTETMBEGA1UECgwK
    QXBwbGUgSW5jLjETMBEGA1UECAwKQ2FsaWZvcm5pYTAeFw0yMDAzMTgxODMyNTNa
    Fw00NTAzMTUwMDAwMDBaMFIxJjAkBgNVBAMMHUFwcGxlIEFwcCBBdHRlc3RhdGlv
    biBSb290IENBMRMwEQYDVQQKDApBcHBsZSBJbmMuMRMwEQYDVQQIDApDYWxpZm9y
    bmlhMHYwEAYHKoZIzj0CAQYFK4EEACIDYgAERTHhmLW07ATaFQIEVwTtT4dyctdh
    NbJhFs/Ii2FdCgAHGbpphY3+d8qjuDngIN3WVhQUBHAoMeQ/cLiP1sOUtgjqK9au
    Yen1mMEvRq9Sk3Jm5X8U62H+xTD3FE9TgS41o0IwQDAPBgNVHRMBAf8EBTADAQH/
    MB0GA1UdDgQWBBSskRBTM72+aEH/pwyp5frq5eWKoTAOBgNVHQ8BAf8EBAMCAQYw
    CgYIKoZIzj0EAwMDaAAwZQIwQgFGnByvsiVbpTKwSga0kP0e8EeDS4+sQmTvb7vn
    53O5+FRXgeLhpJ06ysC5PrOyAjEAp5U4xDgEgllF7En3VcE3iexZZtKeYnpqtijV
    oyFraWVIyd/dganmrduC1bmTBGwD
    -----END CERTIFICATE-----
    """

    private static let platform = "ios"
    private static let formatAppleAppAttest = "apple-appattest"
    private static let keyFormat = "fmt"
    private static let keyAuthData = "authData"
    private static let keyAttStmt = "attStmt"
    private static let keyX5c = "x5c"
    private static let keyAuthenticatorData = "authenticatorData"
    private static let keySignature = "signature"
    private static let appleNonceOID = "1.2.840.113635.100.8.2"
    private static let environmentProduction = "production"
    private static let environmentDevelopment = "development"
    private static let rpIdHashSize = 32
    private static let flagsOffset = 32
    private static let signCountOffset = 33
    private static let aaguidSize = 16
    private static let attestationAuthDataFixedSize = 37
    private static let attestationAuthDataPrefixSize = attestationAuthDataFixedSize + aaguidSize + 2
    private static let assertionAuthDataSize = 37
    private static let flagAttestedCredentialData = 0x40
}

public extension IosOfflineProofVerifier {
    func captureAttestationRpIdHashHex(_ attestationReportBase64: String) throws -> String {
        let attestation = try decodeAttestationObject(attestationReportBase64)
        let authData = try parseAttestationAuthData(attestation.authData)
        return authData.rpIdHash.hexStringLowercased()
    }
}

public struct AndroidOfflineProofVerifier {
    private let trustedRoots: [SecCertificate]
    private let allowInsecureUITestBindings: Bool

    public init() {
        self.init(
            trustedRoots: Self.defaultTrustedRoots,
            allowInsecureUITestBindings: Self.defaultAllowInsecureUITestBindings()
        )
    }

    public init(allowInsecureUITestBindings: Bool) {
        self.init(
            trustedRoots: Self.defaultTrustedRoots,
            allowInsecureUITestBindings: allowInsecureUITestBindings
        )
    }

    public init(
        trustedRoots: [SecCertificate],
        allowInsecureUITestBindings: Bool = false
    ) {
        self.trustedRoots = trustedRoots
        self.allowInsecureUITestBindings = allowInsecureUITestBindings
    }

    public func verifyDeviceBinding(_ binding: ToriiOfflineDeviceBinding) throws {
        guard binding.platform.caseInsensitiveCompare(Self.platform) == .orderedSame else {
            throw OfflineProofVerifierError.invalidBinding("Unsupported offline device binding platform.")
        }
        guard !binding.attestationKeyId.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
              !binding.deviceId.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
              !binding.offlinePublicKey.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
              !binding.attestationReportBase64.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw OfflineProofVerifierError.invalidBinding("Offline device binding is incomplete.")
        }
        #if DEBUG
        if allowInsecureUITestBindings,
           try isUITestInsecureBinding(binding) {
            return
        }
        #endif
        let certificates = try decodeCertificateChain(binding.attestationReportBase64)
        let resolvedChain = try OfflineProofVerifierSupport.verifyCertificateChain(
            certificates: certificates,
            trustedRoots: trustedRoots,
            untrustedChainMessage: "Offline device binding attestation report is empty.",
            rootTrustMessage: "Offline device binding root certificate is not trusted."
        )
        guard let attestationCertificate = resolvedChain.reversed().first(where: { certificate in
            (try? OfflineProofVerifierSupport.extensionValue(
                oid: Self.keyAttestationOID,
                certificateDER: certificate.der,
                missingMessage: "",
                invalidMessage: ""
            )) != nil
        }) else {
            throw OfflineProofVerifierError.invalidBinding(
                "Offline device binding is missing the Android attestation extension."
            )
        }
        let keyDescription = try parseKeyDescription(attestationCertificate.der)
        guard isHardwareSecurityLevel(keyDescription.attestationSecurityLevel),
              isHardwareSecurityLevel(keyDescription.keyMintSecurityLevel) else {
            throw OfflineProofVerifierError.invalidBinding("Offline device binding is not hardware-backed.")
        }
        let expectedPublicKey = try OfflineProofVerifierSupport.decodeRawEd25519PublicKey(
            binding.offlinePublicKey,
            error: "Offline device binding public key is invalid."
        )
        let attestedPublicKey = try OfflineProofVerifierSupport.subjectPublicKeyBytes(
            certificateDER: attestationCertificate.der,
            invalidMessage: "Offline device binding key does not match the attested key."
        )
        guard expectedPublicKey == attestedPublicKey else {
            throw OfflineProofVerifierError.invalidBinding(
                "Offline device binding key does not match the attested key."
            )
        }
        let expectedKeyId = OfflineProofVerifierSupport.sha256Hex(expectedPublicKey)
        guard binding.attestationKeyId.caseInsensitiveCompare(expectedKeyId) == .orderedSame else {
            throw OfflineProofVerifierError.invalidBinding(
                "Offline device binding attestation key id is invalid."
            )
        }
    }

    #if DEBUG
    private func isUITestInsecureBinding(_ binding: ToriiOfflineDeviceBinding) throws -> Bool {
        let report = try OfflineProofVerifierSupport.decodeBase64(
            binding.attestationReportBase64,
            error: "Offline device binding attestation report is invalid."
        )
        guard let reportString = String(data: report, encoding: .utf8),
              reportString.hasPrefix(Self.uitestInsecureReportPrefix) else {
            return false
        }
        let expectedPublicKey = try OfflineProofVerifierSupport.decodeRawEd25519PublicKey(
            binding.offlinePublicKey,
            error: "Offline device binding public key is invalid."
        )
        let expectedKeyId = OfflineProofVerifierSupport.sha256Hex(expectedPublicKey)
        guard binding.attestationKeyId.caseInsensitiveCompare(expectedKeyId) == .orderedSame else {
            throw OfflineProofVerifierError.invalidBinding(
                "Offline device binding attestation key id is invalid."
            )
        }
        return true
    }
    #endif

    public func verifyDeviceProof(
        binding: ToriiOfflineDeviceBinding,
        proof: ToriiOfflineDeviceProof
    ) throws {
        guard proof.platform.caseInsensitiveCompare(Self.platform) == .orderedSame else {
            throw OfflineProofVerifierError.invalidProof("Unsupported offline device proof platform.")
        }
        guard binding.attestationKeyId.caseInsensitiveCompare(proof.attestationKeyId) == .orderedSame else {
            throw OfflineProofVerifierError.invalidProof(
                "Offline device proof does not match the device binding."
            )
        }
        let challengeBytes = try OfflineProofVerifierSupport.decodeHexDigest(
            proof.challengeHashHex,
            error: "Offline device proof challenge hash is invalid."
        )
        let signatureBytes = try OfflineProofVerifierSupport.decodeBase64(
            proof.assertionBase64,
            error: "Offline device proof assertion is invalid."
        )
        let publicKey = try OfflineProofVerifierSupport.decodeRawEd25519PublicKey(
            binding.offlinePublicKey,
            error: "Offline device binding public key is invalid."
        )
        guard try OfflineProofVerifierSupport.verifyEd25519Signature(
            payload: challengeBytes,
            signature: signatureBytes,
            rawPublicKey: publicKey
        ) else {
            throw OfflineProofVerifierError.invalidProof("Offline device proof assertion is invalid.")
        }
    }

    private func decodeCertificateChain(_ value: String) throws -> [Data] {
        let cbor = try OfflineProofVerifierSupport.decodeBase64(
            value,
            error: "Offline device binding attestation report is invalid."
        )
        var reader = OfflineProofCBORReader(data: cbor)
        guard let array = try reader.readValue().arrayValue, reader.isAtEnd else {
            throw OfflineProofVerifierError.invalidBinding(
                "Offline device binding attestation report is not a CBOR array."
            )
        }
        let certificates = try array.map { item -> Data in
            guard let certificate = item.byteStringValue else {
                throw OfflineProofVerifierError.invalidBinding(
                    "Offline device binding attestation report contains a non-certificate item."
                )
            }
            return certificate
        }
        guard !certificates.isEmpty else {
            throw OfflineProofVerifierError.invalidBinding(
                "Offline device binding attestation report is empty."
            )
        }
        return certificates
    }

    private func parseKeyDescription(_ certificateDER: Data) throws -> KeyDescription {
        let extensionValue = try OfflineProofVerifierSupport.extensionValue(
            oid: Self.keyAttestationOID,
            certificateDER: certificateDER,
            missingMessage: "Offline device binding is missing the Android attestation extension.",
            invalidMessage: "Offline device binding attestation extension is truncated."
        )
        let sequence = try OfflineProofVerifierSupport.parseSingleASN1Node(
            extensionValue,
            invalidMessage: "Offline device binding attestation extension is truncated."
        )
        guard sequence.tagClass == .universal, sequence.tagNumber == 16, sequence.children.count >= 4 else {
            throw OfflineProofVerifierError.invalidBinding(
                "Offline device binding attestation extension is truncated."
            )
        }
        return KeyDescription(
            attestationSecurityLevel: try sequence.children[1].integerValue(
                invalidMessage: "Unexpected ASN.1 integer type in Android attestation extension."
            ),
            keyMintSecurityLevel: try sequence.children[3].integerValue(
                invalidMessage: "Unexpected ASN.1 integer type in Android attestation extension."
            )
        )
    }

    private func isHardwareSecurityLevel(_ value: Int) -> Bool {
        value == Self.securityLevelTrustedEnvironment || value == Self.securityLevelStrongBox
    }

    private struct KeyDescription {
        let attestationSecurityLevel: Int
        let keyMintSecurityLevel: Int
    }

    private static func defaultAllowInsecureUITestBindings() -> Bool {
#if DEBUG
        let environment = ProcessInfo.processInfo.environment
        let arguments = ProcessInfo.processInfo.arguments
        return environment["XCTestConfigurationFilePath"] != nil
            || environment["XCTestBundlePath"] != nil
            || environment["XCTestSessionIdentifier"] != nil
            || environment.keys.contains(where: { $0.hasPrefix("UITEST_") })
            || arguments.contains(where: { $0.hasPrefix("UITEST_") || $0.hasPrefix("uitest-") })
#else
        return false
#endif
    }

    private static let defaultTrustedRoots: [SecCertificate] = [
        OfflineProofVerifierSupport.pemCertificate(androidKeyAttestationRootCAPEM),
        OfflineProofVerifierSupport.pemCertificate(androidKeyAttestationCAPEM),
    ].compactMap { $0 }

    private static let platform = "android"
    private static let uitestInsecureReportPrefix = "e2e-offline-insecure:"
    private static let keyAttestationOID = "1.3.6.1.4.1.11129.2.1.17"
    private static let securityLevelTrustedEnvironment = 1
    private static let securityLevelStrongBox = 2
    private static let androidKeyAttestationRootCAPEM = """
    -----BEGIN CERTIFICATE-----
    MIIFHDCCAwSgAwIBAgIJAPHBcqaZ6vUdMA0GCSqGSIb3DQEBCwUAMBsxGTAXBgNV
    BAUTEGY5MjAwOWU4NTNiNmIwNDUwHhcNMjIwMzIwMTgwNzQ4WhcNNDIwMzE1MTgw
    NzQ4WjAbMRkwFwYDVQQFExBmOTIwMDllODUzYjZiMDQ1MIICIjANBgkqhkiG9w0B
    AQEFAAOCAg8AMIICCgKCAgEAr7bHgiuxpwHsK7Qui8xUFmOr75gvMsd/dTEDDJdS
    Sxtf6An7xyqpRR90PL2abxM1dEqlXnf2tqw1Ne4Xwl5jlRfdnJLmN0pTy/4lj4/7
    tv0Sk3iiKkypnEUtR6WfMgH0QZfKHM1+di+y9TFRtv6y//0rb+T+W8a9nsNL/ggj
    nar86461qO0rOs2cXjp3kOG1FEJ5MVmFmBGtnrKpa73XpXyTqRxB/M0n1n/W9nGq
    C4FSYa04T6N5RIZGBN2z2MT5IKGbFlbC8UrW0DxW7AYImQQcHtGl/m00QLVWutHQ
    oVJYnFPlXTcHYvASLu+RhhsbDmxMgJJ0mcDpvsC4PjvB+TxywElgS70vE0XmLD+O
    JtvsBslHZvPBKCOdT0MS+tgSOIfga+z1Z1g7+DVagf7quvmag8jfPioyKvxnK/Eg
    sTUVi2ghzq8wm27ud/mIM7AY2qEORR8Go3TVB4HzWQgpZrt3i5MIlCaY504LzSRi
    igHCzAPlHws+W0rB5N+er5/2pJKnfBSDiCiFAVtCLOZ7gLiMm0jhO2B6tUXHI/+M
    RPjy02i59lINMRRev56GKtcd9qO/0kUJWdZTdA2XoS82ixPvZtXQpUpuL12ab+9E
    aDK8Z4RHJYYfCT3Q5vNAXaiWQ+8PTWm2QgBR/bkwSWc+NpUFgNPN9PvQi8WEg5Um
    AGMCAwEAAaNjMGEwHQYDVR0OBBYEFDZh4QB8iAUJUYtEbEf/GkzJ6k8SMB8GA1Ud
    IwQYMBaAFDZh4QB8iAUJUYtEbEf/GkzJ6k8SMA8GA1UdEwEB/wQFMAMBAf8wDgYD
    VR0PAQH/BAQDAgIEMA0GCSqGSIb3DQEBCwUAA4ICAQB8cMqTllHc8U+qCrOlg3H7
    174lmaCsbo/bJ0C17JEgMLb4kvrqsXZs01U3mB/qABg/1t5Pd5AORHARs1hhqGIC
    W/nKMav574f9rZN4PC2ZlufGXb7sIdJpGiO9ctRhiLuYuly10JccUZGEHpHSYM2G
    tkgYbZba6lsCPYAAP83cyDV+1aOkTf1RCp/lM0PKvmxYN10RYsK631jrleGdcdkx
    oSK//mSQbgcWnmAEZrzHoF1/0gso1HZgIn0YLzVhLSA/iXCX4QT2h3J5z3znluKG
    1nv8NQdxei2DIIhASWfu804CA96cQKTTlaae2fweqXjdN1/v2nqOhngNyz1361mF
    mr4XmaKH/ItTwOe72NI9ZcwS1lVaCvsIkTDCEXdm9rCNPAY10iTunIHFXRh+7KPz
    lHGewCq/8TOohBRn0/NNfh7uRslOSZ/xKbN9tMBtw37Z8d2vvnXq/YWdsm1+JLVw
    n6yYD/yacNJBlwpddla8eaVMjsF6nBnIgQOf9zKSe06nSTqvgwUHosgOECZJZ1Eu
    zbH4yswbt02tKtKEFhx+v+OTge/06V+jGsqTWLsfrOCNLuA8H++z+pUENmpqnnHo
    vaI47gC+TNpkgYGkkBT6B/m/U01BuOBBTzhIlMEZq9qkDWuM2cA5kW5V3FJUcfHn
    w1IdYIg2Wxg7yHcQZemFQg==
    -----END CERTIFICATE-----
    """
    private static let androidKeyAttestationCAPEM = """
    -----BEGIN CERTIFICATE-----
    MIICIjCCAaigAwIBAgIRAISp0Cl7DrWK5/8OgN52BgUwCgYIKoZIzj0EAwMwUjEc
    MBoGA1UEAwwTS2V5IEF0dGVzdGF0aW9uIENBMTEQMA4GA1UECwwHQW5kcm9pZDET
    MBEGA1UECgwKR29vZ2xlIExMQzELMAkGA1UEBhMCVVMwHhcNMjUwNzE3MjIzMjE4
    WhcNMzUwNzE1MjIzMjE4WjBSMRwwGgYDVQQDDBNLZXkgQXR0ZXN0YXRpb24gQ0Ex
    MRAwDgYDVQQLDAdBbmRyb2lkMRMwEQYDVQQKDApHb29nbGUgTExDMQswCQYDVQQG
    EwJVUzB2MBAGByqGSM49AgEGBSuBBAAiA2IABCPaI3FO3z5bBQo8cuiEas4HjqCt
    G/mLFfRT0MsIssPBEEU5Cfbt6sH5yOAxqEi5QagpU1yX4HwnGb7OtBYpDTB57uH5
    Eczm34A5FNijV3s0/f0UPl7zbJcTx6xwqMIRq6NCMEAwDwYDVR0TAQH/BAUwAwEB
    /zAOBgNVHQ8BAf8EBAMCAQYwHQYDVR0OBBYEFFIyuyz7RkOb3NaBqQ5lZuA0QepA
    MAoGCCqGSM49BAMDA2gAMGUCMETfjPO/HwqReR2CS7p0ZWoD/LHs6hDi422opifH
    EUaYLxwGlT9SLdjkVpz0UUOR5wIxAIoGyxGKRHVTpqpGRFiJtQEOOTp/+s1GcxeY
    uR2zh/80lQyu9vAFCj6E4AXc+osmRg==
    -----END CERTIFICATE-----
    """
}

private enum ASN1TagClass {
    case universal
    case application
    case contextSpecific
    case `private`
}

private struct OfflineProofASN1Node {
    let tagClass: ASN1TagClass
    let tagNumber: Int
    let isConstructed: Bool
    let encoded: Data
    let primitiveValue: Data
    let children: [OfflineProofASN1Node]

    func integerValue(invalidMessage: String) throws -> Int {
        guard tagClass == .universal, tagNumber == 2 || tagNumber == 10 else {
            throw OfflineProofVerifierError.invalidBinding(invalidMessage)
        }
        guard !primitiveValue.isEmpty else {
            throw OfflineProofVerifierError.invalidBinding(invalidMessage)
        }
        var value = 0
        for byte in primitiveValue {
            value = (value << 8) | Int(byte)
        }
        return value
    }
}

private struct OfflineProofASN1Reader {
    private let data: Data
    private var offset: Int = 0

    init(data: Data) {
        self.data = data
    }

    mutating func readNode(invalidMessage: String) throws -> OfflineProofASN1Node {
        guard offset < data.count else {
            throw OfflineProofVerifierError.invalidBinding(invalidMessage)
        }
        let start = offset
        let first = data[offset]
        offset += 1
        let tagClass = Self.tagClass(for: first)
        let isConstructed = (first & 0x20) != 0
        let tagNumber = Int(first & 0x1F)
        guard tagNumber != 0x1F else {
            throw OfflineProofVerifierError.invalidBinding(invalidMessage)
        }
        let length = try readLength(invalidMessage: invalidMessage)
        let end = offset + length
        guard end <= data.count else {
            throw OfflineProofVerifierError.invalidBinding(invalidMessage)
        }
        let value = data.subdata(in: offset ..< end)
        offset = end
        let children: [OfflineProofASN1Node]
        if isConstructed {
            var childReader = OfflineProofASN1Reader(data: value)
            var parsedChildren: [OfflineProofASN1Node] = []
            while !childReader.isAtEnd {
                parsedChildren.append(try childReader.readNode(invalidMessage: invalidMessage))
            }
            children = parsedChildren
        } else {
            children = []
        }
        return OfflineProofASN1Node(
            tagClass: tagClass,
            tagNumber: tagNumber,
            isConstructed: isConstructed,
            encoded: data.subdata(in: start ..< end),
            primitiveValue: value,
            children: children
        )
    }

    var isAtEnd: Bool {
        offset == data.count
    }

    private mutating func readLength(invalidMessage: String) throws -> Int {
        guard offset < data.count else {
            throw OfflineProofVerifierError.invalidBinding(invalidMessage)
        }
        let first = Int(data[offset])
        offset += 1
        if (first & 0x80) == 0 {
            return first
        }
        let byteCount = first & 0x7F
        guard byteCount > 0, byteCount <= 4, offset + byteCount <= data.count else {
            throw OfflineProofVerifierError.invalidBinding(invalidMessage)
        }
        var length = 0
        for _ in 0 ..< byteCount {
            length = (length << 8) | Int(data[offset])
            offset += 1
        }
        return length
    }

    private static func tagClass(for byte: UInt8) -> ASN1TagClass {
        switch byte >> 6 {
        case 0: return .universal
        case 1: return .application
        case 2: return .contextSpecific
        default: return .private
        }
    }
}

private enum OfflineProofVerifierSupport {
    struct ResolvedCertificate {
        let certificate: SecCertificate
        let der: Data
    }

    struct CertificateSignatureComponents {
        let tbsCertificate: Data
        let signatureAlgorithmOID: String
        let signatureBytes: Data
    }

    static func decodeBase64(_ value: String, error: String) throws -> Data {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw OfflineProofVerifierError.invalidBinding(error)
        }
        if let data = Data(base64Encoded: trimmed) {
            return data
        }
        if let data = Data(base64URLEncoded: trimmed) {
            return data
        }
        throw OfflineProofVerifierError.invalidBinding(error)
    }

    static func decodeCanonicalBase64(_ value: String, error: String) throws -> Data {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, trimmed == value,
              let decoded = Data(base64Encoded: trimmed),
              decoded.base64EncodedString() == trimmed else {
            throw OfflineProofVerifierError.invalidBinding(error)
        }
        return decoded
    }

    static func decodeHexDigest(_ value: String, error: String) throws -> Data {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        guard trimmed.count == 64 else {
            throw OfflineProofVerifierError.invalidProof(error)
        }
        var bytes = Data(capacity: 32)
        var index = trimmed.startIndex
        while index < trimmed.endIndex {
            let next = trimmed.index(index, offsetBy: 2)
            guard let byte = UInt8(trimmed[index ..< next], radix: 16) else {
                throw OfflineProofVerifierError.invalidProof(error)
            }
            bytes.append(byte)
            index = next
        }
        return bytes
    }

    static func readUInt16(_ data: Data, offset: Int) throws -> Int {
        guard offset + 2 <= data.count else {
            throw OfflineProofVerifierError.invalidBinding("Offline device binding authData is truncated.")
        }
        return (Int(data[offset]) << 8) | Int(data[offset + 1])
    }

    static func readUInt32(_ data: Data, offset: Int) throws -> UInt64 {
        guard offset + 4 <= data.count else {
            throw OfflineProofVerifierError.invalidProof("Offline device proof assertion is invalid.")
        }
        return (UInt64(data[offset]) << 24)
            | (UInt64(data[offset + 1]) << 16)
            | (UInt64(data[offset + 2]) << 8)
            | UInt64(data[offset + 3])
    }

    static func verifyCertificateChain(
        certificates: [Data],
        trustedRoots: [SecCertificate],
        untrustedChainMessage: String,
        rootTrustMessage: String
    ) throws -> [ResolvedCertificate] {
        guard !certificates.isEmpty else {
            throw OfflineProofVerifierError.invalidBinding(untrustedChainMessage)
        }
        let resolved = try certificates.map { data -> ResolvedCertificate in
            guard let certificate = SecCertificateCreateWithData(nil, data as CFData) else {
                throw OfflineProofVerifierError.invalidBinding(untrustedChainMessage)
            }
            return ResolvedCertificate(certificate: certificate, der: data)
        }
        var trust: SecTrust?
        let status = SecTrustCreateWithCertificates(
            resolved.map(\.certificate) as CFArray,
            SecPolicyCreateBasicX509(),
            &trust
        )
        guard status == errSecSuccess, let trust else {
            throw OfflineProofVerifierError.invalidBinding(untrustedChainMessage)
        }
        SecTrustSetAnchorCertificates(trust, trustedRoots as CFArray)
        SecTrustSetAnchorCertificatesOnly(trust, true)
        SecTrustSetNetworkFetchAllowed(trust, false)
        var error: CFError?
        if SecTrustEvaluateWithError(trust, &error) {
            return resolved
        }
        #if DEBUG
        if try explicitPinnedChainVerification(resolved: resolved, trustedRoots: trustedRoots) {
            return resolved
        }
        #endif
        throw OfflineProofVerifierError.invalidBinding(rootTrustMessage)
    }

    static func extensionValue(
        oid: String,
        certificateDER: Data,
        missingMessage: String,
        invalidMessage: String
    ) throws -> Data {
        let certificate = try parseSingleASN1Node(certificateDER, invalidMessage: invalidMessage)
        guard certificate.tagClass == .universal,
              certificate.tagNumber == 16,
              certificate.children.count >= 1
        else {
            throw OfflineProofVerifierError.invalidBinding(invalidMessage)
        }
        let tbsCertificate = certificate.children[0]
        guard tbsCertificate.tagClass == .universal,
              tbsCertificate.tagNumber == 16
        else {
            throw OfflineProofVerifierError.invalidBinding(invalidMessage)
        }
        guard let extensionContainer = tbsCertificate.children.first(where: {
            $0.tagClass == .contextSpecific && $0.tagNumber == 3
        }), let extensionsSequence = extensionContainer.children.first
        else {
            throw OfflineProofVerifierError.invalidBinding(missingMessage)
        }
        for entry in extensionsSequence.children {
            guard entry.tagClass == .universal,
                  entry.tagNumber == 16,
                  entry.children.count >= 2
            else {
                throw OfflineProofVerifierError.invalidBinding(invalidMessage)
            }
            guard try decodeOID(entry.children[0], invalidMessage: invalidMessage) == oid else {
                continue
            }
            guard let octetString = entry.children.last,
                  octetString.tagClass == .universal,
                  octetString.tagNumber == 4
            else {
                throw OfflineProofVerifierError.invalidBinding(invalidMessage)
            }
            return octetString.primitiveValue
        }
        throw OfflineProofVerifierError.invalidBinding(missingMessage)
    }

    static func subjectPublicKeyBytes(
        certificateDER: Data,
        invalidMessage: String
    ) throws -> Data {
        let certificate = try parseSingleASN1Node(certificateDER, invalidMessage: invalidMessage)
        guard certificate.tagClass == .universal,
              certificate.tagNumber == 16,
              let tbsCertificate = certificate.children.first,
              tbsCertificate.tagClass == .universal,
              tbsCertificate.tagNumber == 16
        else {
            throw OfflineProofVerifierError.invalidBinding(invalidMessage)
        }
        var children = tbsCertificate.children
        if let first = children.first,
           first.tagClass == .contextSpecific,
           first.tagNumber == 0 {
            children.removeFirst()
        }
        guard children.count >= 6 else {
            throw OfflineProofVerifierError.invalidBinding(invalidMessage)
        }
        let subjectPublicKeyInfo = children[5]
        guard subjectPublicKeyInfo.tagClass == .universal,
              subjectPublicKeyInfo.tagNumber == 16,
              subjectPublicKeyInfo.children.count == 2
        else {
            throw OfflineProofVerifierError.invalidBinding(invalidMessage)
        }
        let bitString = subjectPublicKeyInfo.children[1]
        guard bitString.tagClass == .universal,
              bitString.tagNumber == 3,
              let unusedBits = bitString.primitiveValue.first,
              unusedBits == 0
        else {
            throw OfflineProofVerifierError.invalidBinding(invalidMessage)
        }
        return bitString.primitiveValue.dropFirst()
    }

    static func certificateSignatureComponents(
        certificateDER: Data,
        invalidMessage: String
    ) throws -> CertificateSignatureComponents {
        let certificate = try parseSingleASN1Node(certificateDER, invalidMessage: invalidMessage)
        guard certificate.tagClass == .universal,
              certificate.tagNumber == 16,
              certificate.children.count == 3
        else {
            throw OfflineProofVerifierError.invalidBinding(invalidMessage)
        }
        let tbsCertificate = certificate.children[0]
        let signatureAlgorithm = certificate.children[1]
        let signatureBitString = certificate.children[2]
        guard tbsCertificate.tagClass == .universal,
              tbsCertificate.tagNumber == 16,
              signatureAlgorithm.tagClass == .universal,
              signatureAlgorithm.tagNumber == 16,
              !signatureAlgorithm.children.isEmpty,
              signatureBitString.tagClass == .universal,
              signatureBitString.tagNumber == 3,
              let unusedBits = signatureBitString.primitiveValue.first,
              unusedBits == 0
        else {
            throw OfflineProofVerifierError.invalidBinding(invalidMessage)
        }
        let signatureAlgorithmOID = try decodeOID(signatureAlgorithm.children[0], invalidMessage: invalidMessage)
        return CertificateSignatureComponents(
            tbsCertificate: tbsCertificate.encoded,
            signatureAlgorithmOID: signatureAlgorithmOID,
            signatureBytes: signatureBitString.primitiveValue.dropFirst()
        )
    }

    static func parseSingleASN1Node(_ data: Data, invalidMessage: String) throws -> OfflineProofASN1Node {
        var reader = OfflineProofASN1Reader(data: data)
        let node = try reader.readNode(invalidMessage: invalidMessage)
        guard reader.isAtEnd else {
            throw OfflineProofVerifierError.invalidBinding(invalidMessage)
        }
        return node
    }

    static func decodeRawEd25519PublicKey(_ value: String, error: String) throws -> Data {
        let decoded = try decodeBase64(value, error: error)
        if decoded.count == 32 {
            return decoded
        }
        if decoded.count == 44,
           decoded.prefix(12) == Data([0x30, 0x2A, 0x30, 0x05, 0x06, 0x03, 0x2B, 0x65, 0x70, 0x03, 0x21, 0x00]) {
            return decoded.dropFirst(12)
        }
        throw OfflineProofVerifierError.invalidBinding(error)
    }

    static func verifyEd25519Signature(
        payload: Data,
        signature: Data,
        rawPublicKey: Data
    ) throws -> Bool {
        let publicKey = try Curve25519.Signing.PublicKey(rawRepresentation: rawPublicKey)
        return publicKey.isValidSignature(signature, for: payload)
    }

    static func sha256Hex(_ data: Data) -> String {
        Data(SHA256.hash(data: data)).hexStringLowercased()
    }

    static func concatenate(_ lhs: Data, _ rhs: Data) -> Data {
        var data = Data()
        data.append(lhs)
        data.append(rhs)
        return data
    }

    static func pemCertificate(_ pem: String) -> SecCertificate? {
        let base64 = pem
            .components(separatedBy: .newlines)
            .filter { !$0.hasPrefix("-----BEGIN") && !$0.hasPrefix("-----END") }
            .joined()
        guard let data = Data(base64Encoded: base64) else {
            return nil
        }
        return SecCertificateCreateWithData(nil, data as CFData)
    }

    #if DEBUG
    private static func explicitPinnedChainVerification(
        resolved: [ResolvedCertificate],
        trustedRoots: [SecCertificate]
    ) throws -> Bool {
        guard !resolved.isEmpty, !trustedRoots.isEmpty else {
            return false
        }
        for index in 0 ..< resolved.count - 1 {
            guard try verifyCertificateSignature(
                certificateDER: resolved[index].der,
                issuerCertificate: resolved[index + 1].certificate
            ) else {
                return false
            }
        }
        guard let last = resolved.last else {
            return false
        }
        for root in trustedRoots {
            let rootDER = SecCertificateCopyData(root) as Data
            if rootDER == last.der {
                return true
            }
            if try verifyCertificateSignature(certificateDER: last.der, issuerCertificate: root) {
                return true
            }
        }
        return false
    }

    private static func verifyCertificateSignature(
        certificateDER: Data,
        issuerCertificate: SecCertificate
    ) throws -> Bool {
        let components = try certificateSignatureComponents(
            certificateDER: certificateDER,
            invalidMessage: "Offline device binding certificate signature is invalid."
        )
        guard let issuerKey = SecCertificateCopyKey(issuerCertificate) else {
            return false
        }
        let algorithm = try secKeyAlgorithm(
            forSignatureOID: components.signatureAlgorithmOID,
            invalidMessage: "Offline device binding certificate signature algorithm is unsupported."
        )
        var error: Unmanaged<CFError>?
        return SecKeyVerifySignature(
            issuerKey,
            algorithm,
            components.tbsCertificate as CFData,
            components.signatureBytes as CFData,
            &error
        )
    }

    private static func secKeyAlgorithm(
        forSignatureOID oid: String,
        invalidMessage: String
    ) throws -> SecKeyAlgorithm {
        switch oid {
        case "1.2.840.10045.4.3.2":
            return .ecdsaSignatureMessageX962SHA256
        case "1.2.840.10045.4.3.3":
            return .ecdsaSignatureMessageX962SHA384
        case "1.2.840.10045.4.3.4":
            return .ecdsaSignatureMessageX962SHA512
        case "1.2.840.113549.1.1.11":
            return .rsaSignatureMessagePKCS1v15SHA256
        case "1.2.840.113549.1.1.12":
            return .rsaSignatureMessagePKCS1v15SHA384
        case "1.2.840.113549.1.1.13":
            return .rsaSignatureMessagePKCS1v15SHA512
        default:
            throw OfflineProofVerifierError.invalidBinding(invalidMessage)
        }
    }
    #endif

    private static func decodeOID(
        _ node: OfflineProofASN1Node,
        invalidMessage: String
    ) throws -> String {
        guard node.tagClass == .universal, node.tagNumber == 6, !node.primitiveValue.isEmpty else {
            throw OfflineProofVerifierError.invalidBinding(invalidMessage)
        }
        let bytes = [UInt8](node.primitiveValue)
        var components: [String] = []
        let first = Int(bytes[0])
        components.append(String(first / 40))
        components.append(String(first % 40))
        var value = 0
        for byte in bytes.dropFirst() {
            value = (value << 7) | Int(byte & 0x7F)
            if (byte & 0x80) == 0 {
                components.append(String(value))
                value = 0
            }
        }
        if value != 0 {
            throw OfflineProofVerifierError.invalidBinding(invalidMessage)
        }
        return components.joined(separator: ".")
    }
}

private enum OfflineProofCBORValue {
    case int(Int)
    case text(String)
    case bytes(Data)
    case array([OfflineProofCBORValue])
    case object([String: OfflineProofCBORValue])
    case intObject([Int: OfflineProofCBORValue])

    var intValue: Int? {
        if case .int(let value) = self { return value }
        return nil
    }

    var textValue: String? {
        if case .text(let value) = self { return value }
        return nil
    }

    var byteStringValue: Data? {
        if case .bytes(let value) = self { return value }
        return nil
    }

    var arrayValue: [OfflineProofCBORValue]? {
        if case .array(let value) = self { return value }
        return nil
    }

    var mapValue: [String: OfflineProofCBORValue]? {
        if case .object(let value) = self { return value }
        return nil
    }

    var intMapValue: [Int: OfflineProofCBORValue]? {
        if case .intObject(let value) = self { return value }
        return nil
    }
}

private struct OfflineProofCBORReader {
    private let data: Data
    private var index: Data.Index

    init(data: Data) {
        self.data = data
        index = data.startIndex
    }

    var isAtEnd: Bool {
        index == data.endIndex
    }

    mutating func readValue() throws -> OfflineProofCBORValue {
        let header = try readByte()
        let majorType = header >> 5
        let length = try readLength(from: header)
        switch majorType {
        case 0:
            return .int(length)
        case 1:
            return .int(-1 - length)
        case 2:
            return .bytes(try readBytes(length))
        case 3:
            guard let value = String(data: try readBytes(length), encoding: .utf8) else {
                throw OfflineProofCBORError.invalidFormat
            }
            return .text(value)
        case 4:
            return .array(try (0 ..< length).map { _ in try readValue() })
        case 5:
            var textResult: [String: OfflineProofCBORValue] = [:]
            var intResult: [Int: OfflineProofCBORValue] = [:]
            var keyKind: OfflineProofCBORMapKeyKind?
            for _ in 0 ..< length {
                let keyValue = try readValue()
                switch keyValue {
                case .text(let key):
                    guard keyKind == nil || keyKind == .text else {
                        throw OfflineProofCBORError.invalidFormat
                    }
                    keyKind = .text
                    textResult[key] = try readValue()
                case .int(let key):
                    guard keyKind == nil || keyKind == .int else {
                        throw OfflineProofCBORError.invalidFormat
                    }
                    keyKind = .int
                    intResult[key] = try readValue()
                default:
                    throw OfflineProofCBORError.invalidFormat
                }
            }
            switch keyKind {
            case .text:
                return .object(textResult)
            case .int:
                return .intObject(intResult)
            case nil:
                return .object([:])
            }
        default:
            throw OfflineProofCBORError.invalidFormat
        }
    }

    private mutating func readByte() throws -> UInt8 {
        guard index < data.endIndex else {
            throw OfflineProofCBORError.invalidFormat
        }
        let value = data[index]
        index = data.index(after: index)
        return value
    }

    private mutating func readLength(from header: UInt8) throws -> Int {
        let additional = header & 0x1F
        switch additional {
        case 0 ... 23:
            return Int(additional)
        case 24:
            return Int(try readByte())
        case 25:
            return (Int(try readByte()) << 8) | Int(try readByte())
        case 26:
            return (Int(try readByte()) << 24)
                | (Int(try readByte()) << 16)
                | (Int(try readByte()) << 8)
                | Int(try readByte())
        default:
            throw OfflineProofCBORError.invalidFormat
        }
    }

    private mutating func readBytes(_ count: Int) throws -> Data {
        guard let end = data.index(index, offsetBy: count, limitedBy: data.endIndex) else {
            throw OfflineProofCBORError.invalidFormat
        }
        let bytes = data[index ..< end]
        index = end
        return Data(bytes)
    }
}

private enum OfflineProofCBORMapKeyKind {
    case text
    case int
}

private extension Array where Element == Data? {
    func optionalUnwrap() -> [Data]? {
        if contains(where: { $0 == nil }) {
            return nil
        }
        return compactMap { $0 }
    }
}

private extension Data {
    init?(base64URLEncoded value: String) {
        var normalized = value
            .replacingOccurrences(of: "-", with: "+")
            .replacingOccurrences(of: "_", with: "/")
        let remainder = normalized.count % 4
        if remainder != 0 {
            normalized.append(String(repeating: "=", count: 4 - remainder))
        }
        self.init(base64Encoded: normalized)
    }

    func hexStringLowercased() -> String {
        map { String(format: "%02x", $0) }.joined()
    }
}
