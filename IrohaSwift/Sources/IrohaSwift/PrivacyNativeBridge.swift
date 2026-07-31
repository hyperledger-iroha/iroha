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
            throw PrivacyCompiledProfileCatalogBridgeError.unknownProtocol
        }
        self = value
    }
}

public enum PrivacyCompiledProfileCatalogBridgeError: Error, Equatable, Sendable {
    case nativeUnavailable
    case invalidArchive
    case invalidFixtureBundle
    case unknownProtocol
}

/// Stable ABI-21 result of validating one typed local compiled-profile catalog.
public enum PrivacyCompiledProfileCatalogValidationStatusV1: Int32, CaseIterable, Sendable {
    case valid = 0
    case nullPointer = 1
    case empty = 2
    case archiveTooLarge = 3
    case decodeResourceLimit = 4
    case schemaMismatch = 5
    case nonCanonical = 6
    case malformedArchive = 7
    case invalidCatalog = 8
}

/// Stable ABI-21 result of validating the Rust-derived exact-12 fixture bundle.
public enum PrivacyExact12FixtureValidationStatusV1: Int32, CaseIterable, Sendable {
    case valid = 0
    case nullPointer = 1
    case empty = 2
    case archiveTooLarge = 3
    case decodeResourceLimit = 4
    case schemaMismatch = 5
    case nonCanonical = 6
    case malformedArchive = 7
    case invalidBundle = 8
}

/// Selector-free local build metadata and exact-12 fixture surface.
///
/// Generic proof request/build/verify dispatch is intentionally absent. Each proof protocol owns
/// its typed API. The compiled-profile catalog describes only the current binary and never
/// establishes network activation or readiness. Fetch a fresh authoritative
/// `PrivacyCapabilitySnapshotV1` from live Torii before submitting a privacy proof.
public enum PrivacyNativeBridge {
    public static let requiredBridgeABIVersion: UInt32 = 21
    public static let compiledProfileCatalogArchiveMaximumBytes = 256 * 1024
    public static let exact12FixtureBundleMaximumBytes = 2 * 1024 * 1024

    public static var isNativeAvailable: Bool {
        NoritoNativeBridge.shared.isPrivacyNativeAvailable
    }

    /// All twelve protocol identities in exact wire order.
    public static var protocolsV1: [PrivacyProtocolIdV1] {
        PrivacyProtocolIdV1.allCases
    }

    /// Return this binary's canonical typed Norito compiled-profile catalog.
    public static func compiledProfileCatalogV1() throws -> Data {
        guard isNativeAvailable,
              let archive = try NoritoNativeBridge.shared.privacyCompiledProfileCatalogV1() else {
            throw PrivacyCompiledProfileCatalogBridgeError.nativeUnavailable
        }
        return try requireCompiledProfileCatalogV1(archive)
    }

    /// Validate bytes as the exact compiled-profile catalog of the loaded binary.
    public static func validateCompiledProfileCatalogV1(
        _ archive: Data
    ) throws -> PrivacyCompiledProfileCatalogValidationStatusV1 {
        guard !archive.isEmpty else {
            return .empty
        }
        guard archive.count <= compiledProfileCatalogArchiveMaximumBytes else {
            return .archiveTooLarge
        }
        guard isNativeAvailable,
              let rawStatus = NoritoNativeBridge.shared
                .privacyCompiledProfileCatalogValidationStatusV1(archive) else {
            throw PrivacyCompiledProfileCatalogBridgeError.nativeUnavailable
        }
        guard let status = PrivacyCompiledProfileCatalogValidationStatusV1(
            rawValue: rawStatus
        ) else {
            throw PrivacyCompiledProfileCatalogBridgeError.invalidArchive
        }
        return status
    }

    /// Return canonical Rust-derived bytes through signed-transaction and hash layers for all twelve rows.
    public static func exact12FixtureBundleV1() throws -> Data {
        guard isNativeAvailable,
              let archive = try NoritoNativeBridge.shared.privacyExact12FixtureBundleV1() else {
            throw PrivacyCompiledProfileCatalogBridgeError.nativeUnavailable
        }
        return try requireExact12FixtureBundleV1(archive)
    }

    /// Validate an untrusted exact-12 bundle against the Rust-compiled canonical bytes.
    public static func validateExact12FixtureBundleV1(
        _ archive: Data
    ) throws -> PrivacyExact12FixtureValidationStatusV1 {
        guard !archive.isEmpty else {
            return .empty
        }
        guard archive.count <= exact12FixtureBundleMaximumBytes else {
            return .archiveTooLarge
        }
        guard isNativeAvailable,
              let rawStatus =
              NoritoNativeBridge.shared.privacyExact12FixtureValidationStatusV1(archive) else {
            throw PrivacyCompiledProfileCatalogBridgeError.nativeUnavailable
        }
        guard let status = PrivacyExact12FixtureValidationStatusV1(rawValue: rawStatus) else {
            throw PrivacyCompiledProfileCatalogBridgeError.invalidFixtureBundle
        }
        return status
    }

    static func requireCompiledProfileCatalogV1(_ archive: Data) throws -> Data {
        guard !archive.isEmpty,
              archive.count <= compiledProfileCatalogArchiveMaximumBytes,
              NoritoNativeBridge.shared.privacyCompiledProfileCatalogValidationStatusV1(archive)
                == PrivacyCompiledProfileCatalogValidationStatusV1.valid.rawValue else {
            throw PrivacyCompiledProfileCatalogBridgeError.invalidArchive
        }
        return Data(archive)
    }

    static func requireExact12FixtureBundleV1(_ archive: Data) throws -> Data {
        guard try validateExact12FixtureBundleV1(archive) == .valid else {
            throw PrivacyCompiledProfileCatalogBridgeError.invalidFixtureBundle
        }
        return Data(archive)
    }
}
