import Foundation

public struct OfflineNoteInstanceValues: Equatable, Sendable {
    public static let publicValueCount = 16
    public static let maxInputAmounts = 4
    public static let maxOutputAmounts = 2

    public let publicValues: [UInt64]
    public let inputAmounts: [UInt64]
    public let outputAmounts: [UInt64]

    public init(publicValues: [UInt64], inputAmounts: [UInt64], outputAmounts: [UInt64]) throws {
        guard publicValues.count == Self.publicValueCount else {
            throw OfflineNoteInstanceError.invalidPublicValueCount(publicValues.count)
        }
        guard inputAmounts.count == Self.maxInputAmounts else {
            throw OfflineNoteInstanceError.invalidInputAmountCount(inputAmounts.count)
        }
        guard outputAmounts.count == Self.maxOutputAmounts else {
            throw OfflineNoteInstanceError.invalidOutputAmountCount(outputAmounts.count)
        }
        self.publicValues = publicValues
        self.inputAmounts = inputAmounts
        self.outputAmounts = outputAmounts
    }

    public func publicInstanceColumns() -> [[Data]] {
        publicValues.map { [Self.instanceScalarBytes($0)] }
    }

    public static func instanceScalarBytes(_ value: UInt64) -> Data {
        var out = Data(count: 32)
        var word = value
        for idx in 0..<8 {
            out[idx] = UInt8(word & 0xff)
            word >>= 8
        }
        return out
    }
}

public enum OfflineNoteInstanceError: Error, LocalizedError, Equatable {
    case invalidPublicValueCount(Int)
    case invalidInputAmountCount(Int)
    case invalidOutputAmountCount(Int)
    case invalidCount(label: String, count: Int, max: Int)
    case amountDoesNotFitUInt64(String)
    case negativeAmount(String)
    case amountSumOverflow(String)
    case amountConservationMismatch(input: UInt64, output: UInt64)
    case auditInputCountMismatch(nullifiers: Int, claims: Int)
    case auditOutputCountMismatch(commitments: Int, claims: Int)
    case auditOutputClaimOrderMismatch(index: Int)
    case auditInputCertificateMismatch
    case auditAssetDefinitionMismatch

    public var errorDescription: String? {
        switch self {
        case let .invalidPublicValueCount(count):
            return "Offline public instance count must be 16 (found \(count))."
        case let .invalidInputAmountCount(count):
            return "Offline input amount witness count must be 4 (found \(count))."
        case let .invalidOutputAmountCount(count):
            return "Offline output amount witness count must be 2 (found \(count))."
        case let .invalidCount(label, count, max):
            return "Offline \(label) count \(count) must be in 1...\(max)."
        case let .amountDoesNotFitUInt64(amount):
            return "Offline amount \(amount) does not fit the u64 witness corridor."
        case let .negativeAmount(amount):
            return "Offline amount \(amount) must not be negative."
        case let .amountSumOverflow(label):
            return "Offline \(label) amount sum overflows u64 witness units."
        case let .amountConservationMismatch(input, output):
            return "Offline audit amounts are not conserved: input \(input), output \(output)."
        case let .auditInputCountMismatch(nullifiers, claims):
            return "Offline audit input nullifier count \(nullifiers) must match input claim count \(claims)."
        case let .auditOutputCountMismatch(commitments, claims):
            return "Offline audit output commitment count \(commitments) must match output claim count \(claims)."
        case let .auditOutputClaimOrderMismatch(index):
            return "Offline audit output claim at index \(index) must match the output commitment at the same index."
        case .auditInputCertificateMismatch:
            return "Offline audit input claims must match the sender key certificate."
        case .auditAssetDefinitionMismatch:
            return "Offline audit input and output asset definitions must match."
        }
    }
}

public enum OfflineNoteInstanceBuilder {
    private static let modeRedeem: UInt64 = 1
    private static let modeAudit: UInt64 = 2

    public static func redeemInstanceValues(
        for redemption: OfflineNoteRedeem
    ) throws -> OfflineNoteInstanceValues {
        let inputCount = try validateCount(
            redemption.inputNullifiers.count,
            max: OfflineNoteInstanceValues.maxInputAmounts,
            label: "redemption input"
        )
        let outputCount: UInt64 = 1
        let publicInputsHash = try redemption.publicInputsHash()
        let keyCertificatePayloadHash = try redemption.senderKeyCertificate.payloadHash()
        let issuedClaimHash = try redemption.issuedClaim().claimHash()

        let normalizedAmounts = try normalizedAmountUnits([redemption.amount, redemption.amount])
        let inputSum = normalizedAmounts[0]
        let outputSum = normalizedAmounts[1]
        let publicValues = publicValues(
            publicInputsHash: publicInputsHash,
            mode: modeRedeem,
            inputCount: inputCount,
            outputCount: outputCount,
            inputSum: inputSum,
            outputSum: outputSum,
            inputNullifierSum: hashLimb0Sum(redemption.inputNullifiers),
            outputCommitmentSum: 0,
            keyCertificatePayloadHash: keyCertificatePayloadHash,
            sourceOrToken: redemption.sourceNoteCommitment,
            inputClaimHashSum: hashLimb0(issuedClaimHash),
            outputClaimHashSum: 0
        )
        var inputAmounts = [UInt64](repeating: 0, count: OfflineNoteInstanceValues.maxInputAmounts)
        inputAmounts[0] = inputSum
        var outputAmounts = [UInt64](repeating: 0, count: OfflineNoteInstanceValues.maxOutputAmounts)
        outputAmounts[0] = outputSum
        return try OfflineNoteInstanceValues(
            publicValues: publicValues,
            inputAmounts: inputAmounts,
            outputAmounts: outputAmounts
        )
    }

    public static func auditInstanceValues(
        for audit: OfflineNoteAuditBundle
    ) throws -> OfflineNoteInstanceValues {
        let inputCount = try validateCount(
            audit.inputClaims.count,
            max: OfflineNoteInstanceValues.maxInputAmounts,
            label: "audit input"
        )
        let outputCount = try validateCount(
            audit.outputClaims.count,
            max: OfflineNoteInstanceValues.maxOutputAmounts,
            label: "audit output"
        )
        guard audit.inputNullifiers.count == audit.inputClaims.count else {
            throw OfflineNoteInstanceError.auditInputCountMismatch(
                nullifiers: audit.inputNullifiers.count,
                claims: audit.inputClaims.count
            )
        }
        guard audit.outputCommitments.count == audit.outputClaims.count else {
            throw OfflineNoteInstanceError.auditOutputCountMismatch(
                commitments: audit.outputCommitments.count,
                claims: audit.outputClaims.count
            )
        }
        for (index, pair) in zip(audit.outputCommitments, audit.outputClaims).enumerated() where pair.0 != pair.1.noteCommitment {
            throw OfflineNoteInstanceError.auditOutputClaimOrderMismatch(index: index)
        }

        let publicInputsHash = try audit.publicInputsHash()
        let keyCertificatePayloadHash = try audit.senderKeyCertificate.payloadHash()
        guard audit.inputClaims.allSatisfy({ $0.keyCertificatePayloadHash == keyCertificatePayloadHash }) else {
            throw OfflineNoteInstanceError.auditInputCertificateMismatch
        }
        let inputDefinition = try assetDefinitionBytes(audit.inputClaims[0].assetId)
        let inputAssetsMatch = try audit.inputClaims.allSatisfy {
            try assetDefinitionBytes($0.assetId) == inputDefinition
        }
        let outputAssetsMatch = try audit.outputClaims.allSatisfy {
            try assetDefinitionBytes($0.assetId) == inputDefinition
        }
        guard inputAssetsMatch && outputAssetsMatch else {
            throw OfflineNoteInstanceError.auditAssetDefinitionMismatch
        }
        let inputClaimHashes = try audit.inputClaims.map { try $0.claimHash() }
        let outputClaimHashes = try audit.outputClaims.map {
            try OfflineNoteIssuedClaim.fromAuditOutput($0).claimHash()
        }
        let amountStrings = audit.inputClaims.map(\.amount) + audit.outputClaims.map(\.amount)
        let normalizedAmounts = try normalizedAmountUnits(amountStrings)
        let inputUnits = Array(normalizedAmounts.prefix(audit.inputClaims.count))
        let outputUnits = Array(normalizedAmounts.dropFirst(audit.inputClaims.count))
        let inputSum = try checkedSum(inputUnits, label: "input")
        let outputSum = try checkedSum(outputUnits, label: "output")
        guard inputSum == outputSum else {
            throw OfflineNoteInstanceError.amountConservationMismatch(input: inputSum, output: outputSum)
        }

        var inputAmounts = [UInt64](repeating: 0, count: OfflineNoteInstanceValues.maxInputAmounts)
        for (idx, amount) in inputUnits.enumerated() {
            inputAmounts[idx] = amount
        }
        var outputAmounts = [UInt64](repeating: 0, count: OfflineNoteInstanceValues.maxOutputAmounts)
        for (idx, amount) in outputUnits.enumerated() {
            outputAmounts[idx] = amount
        }

        let values = publicValues(
            publicInputsHash: publicInputsHash,
            mode: modeAudit,
            inputCount: inputCount,
            outputCount: outputCount,
            inputSum: inputSum,
            outputSum: outputSum,
            inputNullifierSum: hashLimb0Sum(audit.inputNullifiers),
            outputCommitmentSum: hashLimb0Sum(audit.outputCommitments),
            keyCertificatePayloadHash: keyCertificatePayloadHash,
            sourceOrToken: audit.tokenId,
            inputClaimHashSum: hashLimb0Sum(inputClaimHashes),
            outputClaimHashSum: hashLimb0Sum(outputClaimHashes)
        )
        return try OfflineNoteInstanceValues(
            publicValues: values,
            inputAmounts: inputAmounts,
            outputAmounts: outputAmounts
        )
    }

    private static func validateCount(_ count: Int, max: Int, label: String) throws -> UInt64 {
        guard count >= 1, count <= max else {
            throw OfflineNoteInstanceError.invalidCount(label: label, count: count, max: max)
        }
        return UInt64(count)
    }

    private static func publicValues(
        publicInputsHash: Data,
        mode: UInt64,
        inputCount: UInt64,
        outputCount: UInt64,
        inputSum: UInt64,
        outputSum: UInt64,
        inputNullifierSum: UInt64,
        outputCommitmentSum: UInt64,
        keyCertificatePayloadHash: Data,
        sourceOrToken: Data,
        inputClaimHashSum: UInt64,
        outputClaimHashSum: UInt64
    ) -> [UInt64] {
        let limbs = hashLimbsLE(publicInputsHash)
        return [
            limbs[0],
            limbs[1],
            limbs[2],
            limbs[3],
            mode,
            inputCount,
            outputCount,
            inputSum,
            outputSum,
            inputNullifierSum,
            outputCommitmentSum,
            hashLimb0(keyCertificatePayloadHash),
            hashLimb0(sourceOrToken),
            inputClaimHashSum,
            outputClaimHashSum,
            0,
        ]
    }

    private static func normalizedAmountUnits(_ amounts: [String]) throws -> [UInt64] {
        let trimmed = try amounts.map(trimmedNumeric)
        let targetScale = trimmed.map(\.scale).max() ?? 0
        return try trimmed.map { numeric in
            guard !numeric.isNegative else {
                throw OfflineNoteInstanceError.negativeAmount(numeric.original)
            }
            let scaleDelta = Int(targetScale - numeric.scale)
            let aligned = numeric.digits + String(repeating: "0", count: scaleDelta)
            guard let value = UInt64(aligned) else {
                throw OfflineNoteInstanceError.amountDoesNotFitUInt64(numeric.original)
            }
            return value
        }
    }

    private static func trimmedNumeric(_ amount: String) throws -> (
        original: String,
        isNegative: Bool,
        scale: UInt32,
        digits: String
    ) {
        let parsed = try OfflineNorito.parseCanonicalNumeric(amount)
        var scale = parsed.scale
        var digits = parsed.digits
        while scale > 0, digits.last == "0" {
            digits.removeLast()
            scale -= 1
        }
        if digits.isEmpty {
            digits = "0"
            scale = 0
        }
        return (amount, parsed.isNegative, scale, digits)
    }

    private static func checkedSum(_ values: [UInt64], label: String) throws -> UInt64 {
        try values.reduce(UInt64(0)) { partial, value in
            let (sum, overflow) = partial.addingReportingOverflow(value)
            guard !overflow else {
                throw OfflineNoteInstanceError.amountSumOverflow(label)
            }
            return sum
        }
    }

    private static func assetDefinitionBytes(_ assetId: String) throws -> Data {
        guard let parsed = OfflineNorito.parsePublicAssetIdLiteral(assetId),
              let definitionBytes = AssetDefinitionAddress.decode(parsed.assetDefinitionId) else {
            throw OfflineNoritoError.invalidAssetId(assetId)
        }
        return definitionBytes
    }

    private static func hashLimb0Sum(_ hashes: [Data]) -> UInt64 {
        hashes.reduce(UInt64(0)) { sum, hash in
            sum &+ hashLimb0(hash)
        }
    }

    private static func hashLimb0(_ hash: Data) -> UInt64 {
        hashLimbsLE(hash)[0]
    }

    private static func hashLimbsLE(_ hash: Data) -> [UInt64] {
        precondition(hash.count == 32)
        var limbs = [UInt64](repeating: 0, count: 4)
        for idx in 0..<4 {
            let start = idx * 8
            var value: UInt64 = 0
            for offset in 0..<8 {
                value |= UInt64(hash[start + offset]) << UInt64(offset * 8)
            }
            limbs[idx] = value
        }
        return limbs
    }
}
