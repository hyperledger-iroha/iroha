import Foundation

public enum OfflineBalanceProofError: Swift.Error {
    case bridgeUnavailable
    case invalidCommitmentLength
    case invalidBlindingLength
    case invalidHex(String)
}

public enum OfflineBalanceProofBuilder {
    public static let commitmentLength = 32
    public static let proofLength = 12385

    /// Convenience tuple containing the updated commitment + Chaum–Pedersen proof.
    public struct Artifacts {
        public let resultingCommitment: Data
        public let proof: Data

        public init(resultingCommitment: Data, proof: Data) {
            self.resultingCommitment = resultingCommitment
            self.proof = proof
        }

        public var resultingCommitmentHex: String {
            OfflineBalanceProofBuilder.hex(from: resultingCommitment)
        }

        public var proofHex: String {
            OfflineBalanceProofBuilder.hex(from: proof)
        }
    }

    public static func advanceCommitment(
        chainId: String,
        claimedDelta: String,
        resultingValue: String,
        initialCommitmentHex: String,
        initialBlindingHex: String,
        resultingBlindingHex: String
    ) throws -> Artifacts {
        let initialCommitment = try data(fromHex: initialCommitmentHex, field: "initialCommitmentHex")
        let initialBlinding = try data(fromHex: initialBlindingHex, field: "initialBlindingHex")
        let resultingBlinding = try data(fromHex: resultingBlindingHex, field: "resultingBlindingHex")
        let resultingCommitment = try updateCommitment(
            claimedDelta: claimedDelta,
            initialCommitment: initialCommitment,
            initialBlinding: initialBlinding,
            resultingBlinding: resultingBlinding
        )
        let proof = try generateProof(
            chainId: chainId,
            claimedDelta: claimedDelta,
            resultingValue: resultingValue,
            initialCommitment: initialCommitment,
            resultingCommitment: resultingCommitment,
            initialBlinding: initialBlinding,
            resultingBlinding: resultingBlinding
        )
        return Artifacts(resultingCommitment: resultingCommitment, proof: proof)
    }

    public static func updateCommitment(
        claimedDelta: String,
        initialCommitment: Data,
        initialBlinding: Data,
        resultingBlinding: Data
    ) throws -> Data {
        guard initialCommitment.count == commitmentLength else {
            throw OfflineBalanceProofError.invalidCommitmentLength
        }
        guard initialBlinding.count == commitmentLength,
              resultingBlinding.count == commitmentLength else {
            throw OfflineBalanceProofError.invalidBlindingLength
        }
        do {
            guard let result = try NoritoNativeBridge.shared.offlineCommitmentUpdate(
                claimedDelta: claimedDelta,
                initialCommitment: initialCommitment,
                initialBlinding: initialBlinding,
                resultingBlinding: resultingBlinding
            ) else {
                throw OfflineBalanceProofError.bridgeUnavailable
            }
            return result
        } catch let bridgeError as NoritoNativeBridge.OfflineCommitmentBridgeError {
            switch bridgeError {
            case .callFailed(let status):
                NSLog("[OfflineBalanceProof] offlineCommitmentUpdate callFailed status=%d, delta=%@, commitLen=%d, blindLen=%d, resBlindLen=%d", status, claimedDelta, initialCommitment.count, initialBlinding.count, resultingBlinding.count)
                throw OfflineBalanceProofError.bridgeUnavailable
            }
        }
    }

    public static func generateProof(
        chainId: String,
        claimedDelta: String,
        resultingValue: String,
        initialCommitment: Data,
        resultingCommitment: Data,
        initialBlinding: Data,
        resultingBlinding: Data
    ) throws -> Data {
        guard initialCommitment.count == commitmentLength,
              resultingCommitment.count == commitmentLength else {
            throw OfflineBalanceProofError.invalidCommitmentLength
        }
        guard initialBlinding.count == commitmentLength,
              resultingBlinding.count == commitmentLength else {
            throw OfflineBalanceProofError.invalidBlindingLength
        }
        do {
            guard let proof = try NoritoNativeBridge.shared.offlineBalanceProof(
                chainId: chainId,
                claimedDelta: claimedDelta,
                resultingValue: resultingValue,
                initialCommitment: initialCommitment,
                resultingCommitment: resultingCommitment,
                initialBlinding: initialBlinding,
                resultingBlinding: resultingBlinding
            ) else {
                throw OfflineBalanceProofError.bridgeUnavailable
            }
            return proof
        } catch let bridgeError as NoritoNativeBridge.OfflineBalanceProofBridgeError {
            switch bridgeError {
            case .callFailed(let status):
                NSLog("[OfflineBalanceProof] offlineBalanceProof callFailed status=%d, chainId=%@, delta=%@, value=%@", status, chainId, claimedDelta, resultingValue)
                throw OfflineBalanceProofError.bridgeUnavailable
            }
        }
    }

    private static func data(fromHex hex: String, field: String) throws -> Data {
        guard let data = Data(hexString: hex) else {
            throw OfflineBalanceProofError.invalidHex(field)
        }
        return data
    }

    private static func hex(from data: Data) -> String {
        data.map { String(format: "%02x", $0) }.joined()
    }
}
