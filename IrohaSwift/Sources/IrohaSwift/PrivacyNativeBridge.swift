import Foundation

/// Closed first-release privacy protocol identity in canonical Norito order.
public enum PrivacyProtocolIdV1: String, CaseIterable, Sendable {
    case zkAcePqAuthorizationV0 = "zk-ace-pq-authorization-v0"
    case anonymousPgcKOutOfNV1 = "anonymous-pgc-k-out-of-n-v1"
    case veRangeTransparentRangeV1 = "verange-transparent-range-v1"
    case irohaZkAmsV1 = "iroha-zk-ams-v1"
    case vegaExistingCredentialZkV0 = "vega-existing-credential-zk-v0"
    case irohaZkX509StarkP256V0 = "iroha-zk-x509-stark-p256-v0"
    case irohaJindoPolynomialCommitmentV0 = "iroha-jindo-polynomial-commitment-v0"
    case irohaBootleLanternAnoncredV1 = "iroha-bootle-lantern-anoncred-v1"
    case orchardHalo2ActionsV1 = "orchard-halo2-actions-v1"
    case moneroFcmpPlusPlusV1 = "monero-fcmp-plus-plus-v1"
    case irohaIvmPrivateNoteStarkV1 = "iroha-ivm-private-note-stark-v1"
    case pqMaspStarkV0 = "pq-masp-stark-v0"

    /// Parse one exact canonical label. Aliases and normalized spellings are rejected.
    public init(canonicalLabel: String) throws {
        guard let value = Self(rawValue: canonicalLabel) else {
            throw PrivacyCapabilityBridgeError.unknownProtocol
        }
        self = value
    }
}

public enum PrivacyCapabilityBridgeError: Error, Equatable, Sendable {
    case nativeUnavailable
    case invalidArchive
    case unknownProtocol
}

/// Stable ABI-21 result of validating one typed privacy capability archive.
public enum PrivacyCapabilityValidationStatusV1: Int32, CaseIterable, Sendable {
    case valid = 0
    case nullPointer = 1
    case empty = 2
    case archiveTooLarge = 3
    case decodeResourceLimit = 4
    case schemaMismatch = 5
    case nonCanonical = 6
    case malformedArchive = 7
    case invalidSnapshot = 8
}

/// Capability-only native privacy surface.
///
/// Generic proof request/build/verify dispatch is intentionally absent. Each proof protocol owns
/// its typed API; this bridge only transports `PrivacyCapabilitySnapshotV1`.
public enum PrivacyNativeBridge {
    public static let requiredBridgeABIVersion: UInt32 = 21
    public static let nativeArchiveMaximumBytes = 256 * 1024

    public static var isNativeAvailable: Bool {
        NoritoNativeBridge.shared.isPrivacyNativeAvailable
    }

    /// All twelve protocol identities in exact wire order.
    public static var protocolsV1: [PrivacyProtocolIdV1] {
        PrivacyProtocolIdV1.allCases
    }

    /// Return the authoritative typed Norito capability snapshot.
    public static func capabilitiesArchiveV1() throws -> Data {
        guard isNativeAvailable,
              let archive = try NoritoNativeBridge.shared.privacyCapabilitiesV1() else {
            throw PrivacyCapabilityBridgeError.nativeUnavailable
        }
        return try requireCapabilitiesArchiveV1(archive)
    }

    static func requireCapabilitiesArchiveV1(_ archive: Data) throws -> Data {
        guard !archive.isEmpty,
              archive.count <= nativeArchiveMaximumBytes,
              NoritoNativeBridge.shared.privacyCapabilityValidationStatusV1(archive)
                == PrivacyCapabilityValidationStatusV1.valid.rawValue else {
            throw PrivacyCapabilityBridgeError.invalidArchive
        }
        return Data(archive)
    }
}
