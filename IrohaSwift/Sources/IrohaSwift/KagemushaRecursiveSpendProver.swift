import Foundation

public enum KagemushaRecursiveSpendProverError: Error, Equatable, LocalizedError {
    case emptyRequestArchive
    case bridgeUnavailable
    case proofRejected

    public var errorDescription: String? {
        switch self {
        case .emptyRequestArchive:
            return "Kagemusha recursive spend request archive must not be empty."
        case .bridgeUnavailable:
            return NoritoNativeBridge.bridgeUnavailableMessage(
                "Kagemusha recursive spend native bridge is unavailable."
            )
        case .proofRejected:
            return "Kagemusha recursive spend request was rejected by the native bridge."
        }
    }
}

public enum KagemushaOfflineSpendMode: String, Equatable {
    case recursiveSpendV1 = "recursive_spend_v1"
    case checkedPrefoldV1 = "checked_prefold_v1"
}

public enum KagemushaRecursiveSpendProver {
    public static var isNativeAvailable: Bool {
        NoritoNativeBridge.shared.isKagemushaRecursiveSpendAvailable
    }

    public static var preferredMode: KagemushaOfflineSpendMode {
        preferredMode(recursiveSpendAvailable: isNativeAvailable)
    }

    public static func preferredMode(recursiveSpendAvailable: Bool) -> KagemushaOfflineSpendMode {
        recursiveSpendAvailable ? .recursiveSpendV1 : .checkedPrefoldV1
    }

    public static func initSpend(requestArchive: Data) throws -> Data {
        try call(
            requestArchive: requestArchive,
            bridgeAvailable: NoritoNativeBridge.shared.isKagemushaRecursiveSpendAvailable
        ) {
            try NoritoNativeBridge.shared.kagemushaRecursiveSpendInit(requestArchive: $0)
        }
    }

    public static func appendSpend(requestArchive: Data) throws -> Data {
        try call(
            requestArchive: requestArchive,
            bridgeAvailable: NoritoNativeBridge.shared.isKagemushaRecursiveSpendAvailable
        ) {
            try NoritoNativeBridge.shared.kagemushaRecursiveSpendAppend(requestArchive: $0)
        }
    }

    public static func verifySpend(requestArchive: Data) throws -> Data {
        try call(
            requestArchive: requestArchive,
            bridgeAvailable: NoritoNativeBridge.shared.isKagemushaRecursiveSpendAvailable
        ) {
            try NoritoNativeBridge.shared.kagemushaRecursiveSpendVerify(requestArchive: $0)
        }
    }

    public static func redeemSpend(requestArchive: Data) throws -> Data {
        try call(
            requestArchive: requestArchive,
            bridgeAvailable: NoritoNativeBridge.shared.isKagemushaRecursiveSpendAvailable
        ) {
            try NoritoNativeBridge.shared.kagemushaRecursiveSpendRedeem(requestArchive: $0)
        }
    }

    static func call(
        requestArchive: Data,
        bridgeAvailable: Bool,
        _ body: (Data) throws -> Data?
    ) throws -> Data {
        guard !requestArchive.isEmpty else {
            throw KagemushaRecursiveSpendProverError.emptyRequestArchive
        }
        guard bridgeAvailable else {
            throw KagemushaRecursiveSpendProverError.bridgeUnavailable
        }
        let archive: Data?
        do {
            archive = try body(requestArchive)
        } catch NativeBridgeError.kagemushaProve {
            throw KagemushaRecursiveSpendProverError.proofRejected
        } catch {
            throw KagemushaRecursiveSpendProverError.proofRejected
        }
        guard let archive else {
            throw KagemushaRecursiveSpendProverError.bridgeUnavailable
        }
        guard !archive.isEmpty else {
            throw KagemushaRecursiveSpendProverError.proofRejected
        }
        return archive
    }
}
