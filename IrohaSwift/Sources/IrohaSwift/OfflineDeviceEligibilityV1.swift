import Foundation

/// Exact protected registration selector for authenticated eligibility issuance.
///
/// The account is intentionally absent. `ToriiCanonicalRequestAuth` is the sole
/// authoritative account identity at the HTTP boundary.
public struct OfflineDeviceEligibilityRequestV1: Equatable, Sendable {
    public let registrationHash: Data
    public let deviceId: String
    public let attestationKeyId: String
    public let requestedTtlMilliseconds: UInt64

    public init(
        registrationHash: Data,
        deviceId: String,
        attestationKeyId: String,
        requestedTtlMilliseconds: UInt64
    ) throws {
        let deviceBytes = Data(deviceId.utf8)
        let keyBytes = Data(attestationKeyId.utf8)
        guard registrationHash.count == 32,
              registrationHash.contains(where: { $0 != 0 }),
              !deviceBytes.isEmpty,
              deviceBytes.count
                <= KagemushaRecursiveSpend.maximumDeviceEligibilityDeviceIdBytesV1,
              deviceId == deviceId.trimmingCharacters(in: .whitespacesAndNewlines),
              !deviceId.unicodeScalars.contains(where: CharacterSet.controlCharacters.contains),
              !keyBytes.isEmpty,
              keyBytes.count
                <= KagemushaRecursiveSpend.maximumDeviceEligibilityAttestationKeyIdBytesV1,
              attestationKeyId
                == attestationKeyId.trimmingCharacters(in: .whitespacesAndNewlines),
              !attestationKeyId.unicodeScalars.contains(
                where: CharacterSet.controlCharacters.contains
              ),
              requestedTtlMilliseconds > 0,
              requestedTtlMilliseconds
                <= KagemushaRecursiveSpend
                    .maximumDeviceEligibilityCredentialTtlMillisecondsV1 else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "offlineDeviceEligibilityRequestV1"
            )
        }
        self.registrationHash = Data(registrationHash)
        self.deviceId = deviceId
        self.attestationKeyId = attestationKeyId
        self.requestedTtlMilliseconds = requestedTtlMilliseconds
    }
}

public enum OfflineDeviceEligibilityOutcomeV1: UInt8, Equatable, Sendable {
    case eligible = 0
    case drainOnly = 1
    case cryptographicallyRejected = 2
}

public enum OfflineDeviceEligibilityReasonV1: UInt8, Equatable, Sendable {
    case policySatisfied = 0
    case cryptographicAttestationRejected = 1
    case policyNotFresh = 2
    case incompleteAttestedProperties = 3
    case unsupportedPreAndroid12Tee = 4
    case vulnerableFirmware = 5
    case permanentlyBlockedDevice = 6
}

public struct OfflineDeviceEligibilityDecisionV1: Equatable, Sendable {
    public let outcome: OfflineDeviceEligibilityOutcomeV1
    public let reason: OfflineDeviceEligibilityReasonV1
    public let matchedRuleIds: [String]
}

public struct OfflineDeviceEligibilityAdmissionProvenanceV1: Equatable, Sendable {
    public let registrationHash: Data
    public let admissionPolicyHash: Data
    public let admissionHeight: UInt64
    public let admissionTransactionHash: Data
}

public struct OfflineDevicePolicyFinalityClaimsV1: Equatable, Sendable {
    public let finalizedBlockHeight: UInt64
    public let finalizedBlockHash: Data
    public let finalizedBlockTimestampMilliseconds: UInt64
    public let finalityEvidenceHash: Data
}

public struct OfflineDeviceEligibilityPolicyClaimsV1: Equatable, Sendable {
    public let policyEpoch: UInt64
    public let policyHash: Data
    public let freshnessDeadlineMilliseconds: UInt64
    public let finality: OfflineDevicePolicyFinalityClaimsV1
}

public struct OfflineDeviceEligibilityCredentialClaimsV1: Equatable, Sendable {
    public let accountId: String
    public let deviceId: String
    public let attestationKeyId: String
    public let devicePublicKey: Data
    public let assertionPublicKey: Data
    public let issuedAtMilliseconds: UInt64
    public let expiresAtMilliseconds: UInt64
}

/// Fully native-verified issuance result. All archives are public protocol
/// values; no secret-bundle, witness, or renderer-owned key bytes are exposed.
public struct OfflineDeviceEligibilityResponseV1: Equatable, Sendable {
    public let noritoArchive: Data
    public let decision: OfflineDeviceEligibilityDecisionV1
    public let issuer: KagemushaEligibilityIssuerPublicKeyV1
    public let credential: KagemushaEligibilityCredentialV1?
    public let finalizedPolicy: KagemushaDeviceAttestationPolicyViewV1
    public let policyClaims: OfflineDeviceEligibilityPolicyClaimsV1
    public let credentialClaims: OfflineDeviceEligibilityCredentialClaimsV1?
    public let admission: OfflineDeviceEligibilityAdmissionProvenanceV1

    static let projectionMagic = Data([0x49, 0x44, 0x45, 0x52, 0x53, 0x50, 0x31, 0])
    static let fixedProjectionBytes = 296

    init(
        nativeProjection: Data,
        responseArchive: Data,
        expectedRegistrationHash: Data,
        trustAnchor: OfflineDeviceFinalityTrustAnchorV1
    ) throws {
        guard nativeProjection.count >= Self.fixedProjectionBytes,
              !responseArchive.isEmpty,
              responseArchive.count
                <= KagemushaRecursiveSpend.maximumDeviceEligibilityResponseArchiveBytesV1,
              nativeProjection.count
                <= KagemushaRecursiveSpend.maximumDeviceEligibilityVerifiedResponseBytesV1,
              nativeProjection.prefix(8) == Self.projectionMagic,
              nativeProjection[11] == 0,
              nativeProjection[118..<120].allSatisfy({ $0 == 0 }),
              nativeProjection[290..<292].allSatisfy({ $0 == 0 }),
              let outcome = OfflineDeviceEligibilityOutcomeV1(
                  rawValue: nativeProjection[8]
              ),
              let reason = OfflineDeviceEligibilityReasonV1(
                  rawValue: nativeProjection[9]
              ),
              nativeProjection[10] <= 1 else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "offlineDeviceEligibilityResponseV1.frame"
            )
        }
        let admissionHeight = Self.readUInt64(nativeProjection, at: 12)
        let registrationHash = Data(nativeProjection[20..<52])
        let admissionPolicyHash = Data(nativeProjection[52..<84])
        let admissionTransactionHash = Data(nativeProjection[84..<116])
        guard admissionHeight > 0,
              registrationHash == expectedRegistrationHash,
              registrationHash.contains(where: { $0 != 0 }),
              admissionPolicyHash.contains(where: { $0 != 0 }),
              admissionTransactionHash.contains(where: { $0 != 0 }) else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "offlineDeviceEligibilityResponseV1.provenance"
            )
        }
        let matchedCount = Int(Self.readUInt16(nativeProjection, at: 116))
        let matchedLength = Int(Self.readUInt32(nativeProjection, at: 120))
        let issuerLength = Int(Self.readUInt32(nativeProjection, at: 124))
        let credentialLength = Int(Self.readUInt32(nativeProjection, at: 128))
        let policyLength = Int(Self.readUInt32(nativeProjection, at: 132))
        let policyEpoch = Self.readUInt64(nativeProjection, at: 136)
        let policyHash = Data(nativeProjection[144..<176])
        let freshnessDeadline = Self.readUInt64(nativeProjection, at: 176)
        let finalizedBlockHeight = Self.readUInt64(nativeProjection, at: 184)
        let finalizedBlockHash = Data(nativeProjection[192..<224])
        let finalizedBlockTimestamp = Self.readUInt64(nativeProjection, at: 224)
        let finalityEvidenceHash = Data(nativeProjection[232..<264])
        let credentialIssuedAt = Self.readUInt64(nativeProjection, at: 264)
        let credentialExpiresAt = Self.readUInt64(nativeProjection, at: 272)
        let claimLengths = [
            Int(Self.readUInt16(nativeProjection, at: 280)),
            Int(Self.readUInt16(nativeProjection, at: 282)),
            Int(Self.readUInt16(nativeProjection, at: 284)),
            Int(Self.readUInt16(nativeProjection, at: 286)),
            Int(Self.readUInt16(nativeProjection, at: 288)),
        ]
        let claimsLength = Int(Self.readUInt32(nativeProjection, at: 292))
        guard policyEpoch > 0,
              policyHash.contains(where: { $0 != 0 }),
              freshnessDeadline > finalizedBlockTimestamp,
              finalizedBlockHeight >= admissionHeight,
              finalizedBlockHash.contains(where: { $0 != 0 }),
              finalizedBlockTimestamp > 0,
              finalityEvidenceHash.contains(where: { $0 != 0 }),
              claimLengths.reduce(0, +) == claimsLength else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "offlineDeviceEligibilityResponseV1.claims"
            )
        }
        let sectionLengths = [
            matchedLength, issuerLength, credentialLength, policyLength, claimsLength,
        ]
        var expectedLength = Self.fixedProjectionBytes
        for length in sectionLengths {
            let addition = expectedLength.addingReportingOverflow(length)
            guard !addition.overflow else {
                throw KagemushaRecursiveSpendError.invalidArchive(
                    "offlineDeviceEligibilityResponseV1.length"
                )
            }
            expectedLength = addition.partialValue
        }
        guard expectedLength == nativeProjection.count,
              issuerLength > 0,
              issuerLength
                <= KagemushaRecursiveSpend.maximumEligibilityIssuerArchiveBytesV1,
              credentialLength
                <= KagemushaRecursiveSpend.maximumEligibilityCredentialArchiveBytesV1,
              policyLength > 0,
              policyLength
                <= KagemushaRecursiveSpend
                    .maximumDeviceAttestationPolicyViewArchiveBytesV1,
              (nativeProjection[10] == 1) == (credentialLength > 0),
              (outcome == .eligible) == (credentialLength > 0),
              (credentialLength == 0)
                == (credentialIssuedAt == 0 && credentialExpiresAt == 0
                    && claimLengths.allSatisfy({ $0 == 0 })) else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "offlineDeviceEligibilityResponseV1.sections"
            )
        }

        var cursor = Self.fixedProjectionBytes
        let matchedEnd = cursor + matchedLength
        let matchedRuleIds = try Self.decodeMatchedRules(
            Data(nativeProjection[cursor..<matchedEnd]),
            expectedCount: matchedCount
        )
        cursor = matchedEnd
        let issuerEnd = cursor + issuerLength
        let issuerArchive = Data(nativeProjection[cursor..<issuerEnd])
        cursor = issuerEnd
        let credentialEnd = cursor + credentialLength
        let credentialArchive = Data(nativeProjection[cursor..<credentialEnd])
        cursor = credentialEnd
        let policyEnd = cursor + policyLength
        let policyArchive = Data(nativeProjection[cursor..<policyEnd])
        cursor = policyEnd
        let claimsBytes = Data(nativeProjection[cursor...])

        try Self.validateDecisionShape(
            outcome: outcome,
            reason: reason,
            matchedRuleIds: matchedRuleIds
        )
        let finalizedPolicy = try KagemushaDeviceAttestationPolicyViewV1(
            validatedArchive: policyArchive,
            trustAnchor: trustAnchor
        )
        let credential: KagemushaEligibilityCredentialV1?
        if credentialArchive.isEmpty {
            credential = nil
            credentialClaims = nil
        } else {
            credential = try KagemushaEligibilityCredentialV1(
                validatedArchive: credentialArchive,
                trustAnchor: trustAnchor
            )
            guard credentialIssuedAt > 0,
                  credentialExpiresAt > credentialIssuedAt,
                  claimLengths[3] == 65,
                  claimLengths[4] == 65 else {
                throw KagemushaRecursiveSpendError.invalidArchive(
                    "offlineDeviceEligibilityResponseV1.credentialClaims"
                )
            }
            var claimCursor = 0
            var claimSections: [Data] = []
            for length in claimLengths {
                let end = claimCursor + length
                guard end <= claimsBytes.count else {
                    throw KagemushaRecursiveSpendError.invalidArchive(
                        "offlineDeviceEligibilityResponseV1.credentialClaims"
                    )
                }
                claimSections.append(Data(claimsBytes[claimCursor..<end]))
                claimCursor = end
            }
            guard claimCursor == claimsBytes.count,
                  let accountId = Self.decodeCanonicalString(claimSections[0]),
                  let credentialDeviceId = Self.decodeCanonicalString(claimSections[1]),
                  let credentialAttestationKeyId = Self.decodeCanonicalString(claimSections[2]),
                  claimSections[3].first == 0x04,
                  claimSections[4].first == 0x04 else {
                throw KagemushaRecursiveSpendError.invalidArchive(
                    "offlineDeviceEligibilityResponseV1.credentialClaims"
                )
            }
            credentialClaims = OfflineDeviceEligibilityCredentialClaimsV1(
                accountId: accountId,
                deviceId: credentialDeviceId,
                attestationKeyId: credentialAttestationKeyId,
                devicePublicKey: claimSections[3],
                assertionPublicKey: claimSections[4],
                issuedAtMilliseconds: credentialIssuedAt,
                expiresAtMilliseconds: credentialExpiresAt
            )
        }
        decision = OfflineDeviceEligibilityDecisionV1(
            outcome: outcome,
            reason: reason,
            matchedRuleIds: matchedRuleIds
        )
        issuer = try KagemushaEligibilityIssuerPublicKeyV1(
            noritoArchive: issuerArchive
        )
        self.credential = credential
        self.finalizedPolicy = finalizedPolicy
        policyClaims = OfflineDeviceEligibilityPolicyClaimsV1(
            policyEpoch: policyEpoch,
            policyHash: policyHash,
            freshnessDeadlineMilliseconds: freshnessDeadline,
            finality: OfflineDevicePolicyFinalityClaimsV1(
                finalizedBlockHeight: finalizedBlockHeight,
                finalizedBlockHash: finalizedBlockHash,
                finalizedBlockTimestampMilliseconds: finalizedBlockTimestamp,
                finalityEvidenceHash: finalityEvidenceHash
            )
        )
        admission = OfflineDeviceEligibilityAdmissionProvenanceV1(
            registrationHash: registrationHash,
            admissionPolicyHash: admissionPolicyHash,
            admissionHeight: admissionHeight,
            admissionTransactionHash: admissionTransactionHash
        )
        noritoArchive = Data(responseArchive)
    }

    private static func readUInt16(_ data: Data, at offset: Int) -> UInt16 {
        data[offset..<(offset + 2)].reduce(UInt16(0)) { ($0 << 8) | UInt16($1) }
    }

    private static func readUInt32(_ data: Data, at offset: Int) -> UInt32 {
        data[offset..<(offset + 4)].reduce(UInt32(0)) { ($0 << 8) | UInt32($1) }
    }

    private static func readUInt64(_ data: Data, at offset: Int) -> UInt64 {
        data[offset..<(offset + 8)].reduce(UInt64(0)) { ($0 << 8) | UInt64($1) }
    }

    private static func decodeMatchedRules(
        _ bytes: Data,
        expectedCount: Int
    ) throws -> [String] {
        var cursor = 0
        var rules: [String] = []
        rules.reserveCapacity(expectedCount)
        while cursor < bytes.count {
            guard cursor + 2 <= bytes.count else {
                throw KagemushaRecursiveSpendError.invalidArchive(
                    "offlineDeviceEligibilityResponseV1.matchedRules"
                )
            }
            let length = Int(readUInt16(bytes, at: cursor))
            cursor += 2
            guard length > 0, cursor + length <= bytes.count,
                  let rule = decodeCanonicalString(Data(bytes[cursor..<(cursor + length)])),
                  rule == rule.trimmingCharacters(in: .whitespacesAndNewlines),
                  !rule.unicodeScalars.contains(
                    where: CharacterSet.controlCharacters.contains
                  ) else {
                throw KagemushaRecursiveSpendError.invalidArchive(
                    "offlineDeviceEligibilityResponseV1.matchedRules"
                )
            }
            rules.append(rule)
            cursor += length
        }
        guard rules.count == expectedCount,
              zip(rules, rules.dropFirst()).allSatisfy({ $0 < $1 }) else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "offlineDeviceEligibilityResponseV1.matchedRules"
            )
        }
        return rules
    }

    private static func decodeCanonicalString(_ data: Data) -> String? {
        guard !data.isEmpty,
              let value = String(data: data, encoding: .utf8),
              Data(value.utf8) == data,
              value == value.trimmingCharacters(in: .whitespacesAndNewlines),
              !value.unicodeScalars.contains(
                where: CharacterSet.controlCharacters.contains
              ) else {
            return nil
        }
        return value
    }

    private static func validateDecisionShape(
        outcome: OfflineDeviceEligibilityOutcomeV1,
        reason: OfflineDeviceEligibilityReasonV1,
        matchedRuleIds: [String]
    ) throws {
        let valid: Bool
        switch (outcome, reason, matchedRuleIds.isEmpty) {
        case (.eligible, .policySatisfied, true),
             (.cryptographicallyRejected, .cryptographicAttestationRejected, true),
             (.drainOnly, .policyNotFresh, true),
             (.drainOnly, .incompleteAttestedProperties, true),
             (.drainOnly, .unsupportedPreAndroid12Tee, true),
             (.drainOnly, .vulnerableFirmware, false),
             (.drainOnly, .permanentlyBlockedDevice, false):
            valid = true
        default:
            valid = false
        }
        guard valid else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "offlineDeviceEligibilityResponseV1.decision"
            )
        }
    }
}

public extension KagemushaRecursiveSpend {
    static func makeOfflineDeviceEligibilityRequestV1(
        _ request: OfflineDeviceEligibilityRequestV1
    ) throws -> Data {
        guard let archive = try NoritoNativeBridge.shared.offlineDeviceEligibilityRequestV1(
            registrationHash: request.registrationHash,
            deviceId: Data(request.deviceId.utf8),
            attestationKeyId: Data(request.attestationKeyId.utf8),
            requestedTtlMilliseconds: request.requestedTtlMilliseconds
        ), !archive.isEmpty,
           archive.count <= maximumDeviceEligibilityRequestArchiveBytesV1 else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        return archive
    }

    static func verifyOfflineDeviceEligibilityResponseV1(
        archive: Data,
        request: OfflineDeviceEligibilityRequestV1,
        expectedIssuer: KagemushaEligibilityIssuerPublicKeyV1,
        trustAnchor: OfflineDeviceFinalityTrustAnchorV1,
        evaluationTimeMilliseconds: UInt64
    ) throws -> OfflineDeviceEligibilityResponseV1 {
        guard evaluationTimeMilliseconds > 0,
              !archive.isEmpty,
              archive.count <= maximumDeviceEligibilityResponseArchiveBytesV1,
              let projection = try NoritoNativeBridge.shared
                .offlineDeviceEligibilityResponseVerifyV1(
                    responseArchive: archive,
                    expectedRegistrationHash: request.registrationHash,
                    expectedIssuerArchive: expectedIssuer.noritoArchive,
                    expectedNetworkId: trustAnchor.networkId.bytes,
                    trustedContextId: trustAnchor.trustedHeightContextId,
                    evaluationTimeMilliseconds: evaluationTimeMilliseconds
                ) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        return try OfflineDeviceEligibilityResponseV1(
            nativeProjection: projection,
            responseArchive: archive,
            expectedRegistrationHash: request.registrationHash,
            trustAnchor: trustAnchor
        )
    }
}
