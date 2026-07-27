import Foundation

/// Low-level proof engines accepted by the canonical
/// `iroha_data_model::zk::BackendTag` Norito wire contract.
///
/// Privacy protocols use their own typed protocol and engine identifiers.
/// They must never be inferred from a generic verifier-backend label.
public enum VerifyingKeyBackendTag: UInt32, CaseIterable, Sendable, Equatable {
    case halo2IpaPasta = 0
    case stark = 1

    public var noritoDiscriminant: UInt32 { rawValue }

    public var canonicalLabel: String {
        switch self {
        case .halo2IpaPasta:
            return "halo2-ipa-pasta"
        case .stark:
            return "stark"
        }
    }

    /// Parse one exact canonical Norito label.
    ///
    /// No whitespace, case, punctuation, or historical aliases are accepted.
    public init?(canonicalLabel: String) {
        switch canonicalLabel {
        case "halo2-ipa-pasta":
            self = .halo2IpaPasta
        case "stark":
            self = .stark
        default:
            return nil
        }
    }
}

public enum VerifierBackendRegistryLabelValidationError: Error, Equatable, LocalizedError {
    case unsupported(context: String, label: String)

    public var errorDescription: String? {
        switch self {
        case let .unsupported(context, label):
            return "\(context) is not an exact supported verifier-registry label: \(label)."
        }
    }
}

/// Exact verifier-registry identifiers accepted by native Rust dispatch.
///
/// These are deliberately separate from `VerifyingKeyBackendTag`: a registry
/// label selects one concrete verifier configuration, while the Norito enum
/// selects its low-level proof engine.
public enum VerifierBackendRegistryLabels {
    private static let supported: Set<String> = [
        "halo2/ipa",
        "halo2/pasta/kaigi-roster-v1",
        "halo2/pasta/kaigi-usage-v1",
        "halo2/pasta/ivm-overlay-bind",
        "halo2/pasta/ivm-execution-v1",
        "halo2/pasta/kagemusha-topup-shield-merkle16-axiom-poseidon-v3",
        "halo2/pasta/kagemusha-recursive-spend-step-eq-two-parent-operation-protocol-v2",
        "halo2/pasta/kagemusha-recursive-spend-step-ep-two-parent-operation-protocol-v2",
        "halo2/pasta/confidential-transfer-2x2-merkle16-axiom-poseidon-v3",
        "halo2/pasta/confidential-unshield-full-merkle16-axiom-poseidon-v3",
        "halo2/pasta/confidential-unshield-change-merkle16-axiom-poseidon-v4",
        "stark/fri",
        "stark/fri/sha256-goldilocks",
        "stark/fri/poseidon2-goldilocks",
        "stark/fri/sha256_goldilocks.v1"
    ]

    public static func isSupported(_ label: String?) -> Bool {
        guard let label else {
            return false
        }
        return supported.contains(label)
    }

    @discardableResult
    public static func requireSupported(
        _ label: String,
        context: String = "backend"
    ) throws -> String {
        guard isSupported(label) else {
            throw VerifierBackendRegistryLabelValidationError.unsupported(
                context: context,
                label: label
            )
        }
        return label
    }
}
